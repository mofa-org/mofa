# Tool Execution Sandbox

Capability-scoped, resource-limited execution for untrusted tools invoked by
LLMs. Tracks GSoC ideas-list Task 34. This document covers the kernel-level
trait contracts and policy model; concrete backends live in
`mofa-foundation`.

---

## Motivation

LLM-called tools execute inside the agent process. Today, a compromised or
malicious tool has the same privileges as the agent: it can read any file
the user can read, open outbound network connections, spawn subprocesses,
and exfiltrate environment variables. That posture is unsustainable once
tools arrive from third-party plugin registries, MCP servers, or prompt-
induced code paths.

The sandbox mediates three questions at every tool call:

```mermaid
flowchart LR
    A[Tool invocation] --> Q1{May tool<br/>read/write<br/>this path?}
    A --> Q2{May tool<br/>reach<br/>this host:port?}
    A --> Q3{Within CPU,<br/>memory, and<br/>wall-time caps?}
    Q1 -- no --> D1[PathNotAllowed]
    Q2 -- no --> D2[NetworkNotAllowed]
    Q3 -- no --> D3[Resource breach]
    Q1 -- yes --> X[Proceed]
    Q2 -- yes --> X
    Q3 -- yes --> X
```

---

## Layered architecture

The kernel owns trait contracts only; foundation owns concrete backends.

```mermaid
flowchart TB
    subgraph Agent [mofa-foundation]
        AL[Agent Loop]
        ST[SandboxedTool&lt;T&gt;]
        AL -->|ToolInput| ST
    end

    subgraph Kernel [mofa-kernel]
        TSB[trait ToolSandbox<br/><br/>policy / tier<br/>precheck / execute]
        ST -->|SandboxRequest| TSB
    end

    subgraph Backends [mofa-foundation backends]
        NB[NullSandbox<br/>Tier: None]
        PB[ProcessSandbox<br/>Tier: Process]
        WB[WasmtimeSandbox<br/>Tier: LanguageVm]
    end

    TSB --> NB
    TSB --> PB
    TSB --> WB
```

---

## Policy model

Policies are default-deny. The base capability set is empty except for
implicit `Compute`; every other capability must be listed and, where
applicable, combined with a fine-grained allow-list.

```mermaid
classDiagram
    class SandboxPolicy {
        +allowed_capabilities: BTreeSet~SandboxCapability~
        +fs_allow_list: Vec~PathPattern~
        +net_allow_list: Vec~NetEndpoint~
        +env_allow_list: Vec~String~
        +subprocess_allow_list: Vec~String~
        +resource_limits: SandboxResourceLimits
        +grants(cap) bool
        +check_fs(tool, path, write) Result
        +check_net(tool, host, port) Result
        +check_env(name) Result
        +check_subprocess(tool, program) Result
        +validate() Result
    }
    class SandboxCapability {
        <<enumeration>>
        FsRead
        FsWrite
        Net
        EnvRead
        Subprocess
        Compute
        Clock
        RandomRead
    }
    class PathPattern {
        <<enumeration>>
        Exact(PathBuf)
        Prefix(PathBuf)
    }
    class NetEndpoint {
        <<enumeration>>
        HostPort
        HostAnyPort
    }
    class SandboxResourceLimits {
        +wall_timeout: Option~Duration~
        +cpu_time_limit: Option~Duration~
        +memory_limit_bytes: Option~u64~
        +output_limit_bytes: Option~u64~
        +max_open_files: Option~u32~
    }
    SandboxPolicy --> SandboxCapability
    SandboxPolicy --> PathPattern
    SandboxPolicy --> NetEndpoint
    SandboxPolicy --> SandboxResourceLimits
```

### Capability gating

| Capability | Fine-grained gate |
|------------|-------------------|
| `FsRead` | `fs_allow_list` (`Vec<PathPattern>`) |
| `FsWrite` | `fs_allow_list` |
| `Net` | `net_allow_list` (`Vec<NetEndpoint>`) |
| `EnvRead` | `env_allow_list` (`Vec<String>`) |
| `Subprocess` | `subprocess_allow_list` (`Vec<String>`) |
| `Compute` | implicit, always granted |
| `Clock` | — |
| `RandomRead` | — |

---

## Execution flow

```mermaid
sequenceDiagram
    participant Caller as SandboxedTool
    participant SB as ToolSandbox impl
    participant Policy as SandboxPolicy
    participant Tool as Tool

    Caller->>SB: execute(SandboxRequest)
    SB->>Policy: precheck(declared_capabilities)
    alt undeclared capability
        Policy-->>SB: Err(CapabilityDenied)
        SB-->>Caller: SandboxError
    else granted
        Policy-->>SB: Ok
        SB->>Tool: run inside isolate
        Tool-->>SB: output
        SB->>Policy: check_fs / check_net / check_env
        alt policy violation
            Policy-->>SB: Err(PathNotAllowed | ...)
            SB-->>Caller: SandboxError
        else resource breach
            SB-->>Caller: Err(WallTimeout | MemoryExceeded | ...)
        else success
            SB-->>Caller: Ok(SandboxResponse { output, stats })
        end
    end
```

---

## Error taxonomy

Three disjoint failure classes with distinct retry semantics.

```mermaid
flowchart LR
    E[SandboxError] --> PD[Policy denial]
    E --> RB[Resource breach]
    E --> BF[Backend failure]

    PD --> PD1[CapabilityDenied]
    PD --> PD2[PathNotAllowed]
    PD --> PD3[NetworkNotAllowed]
    PD --> PD4[EnvVarNotAllowed]
    PD --> PD5[SubprocessNotAllowed]

    RB --> RB1[CpuTimeExceeded]
    RB --> RB2[WallTimeout]
    RB --> RB3[MemoryExceeded]
    RB --> RB4[OutputTooLarge]

    BF --> BF1[BackendFailure]
    BF --> BF2[SandboxCrashed]
    BF --> BF3[IoError]

    PD -.->|not retryable| X[Retry?]
    RB -.->|not retryable| X
    BF -.->|maybe retryable| X
```

`SandboxError::is_policy_denial()`, `is_resource_limit()`, and
`is_backend_failure()` let retry middleware branch on error class.

---

## Tiers of isolation

```mermaid
flowchart TB
    T0[Tier: None<br/>Pass-through<br/>policy checked but not enforced]
    T1[Tier: Process<br/>OS process + rlimit]
    T2[Tier: LanguageVm<br/>Hermetic wasmtime]
    T3[Tier: Virtualized<br/>Containers / microVMs<br/><i>downstream backends</i>]

    T0 --> T1 --> T2 --> T3
```

`SandboxTier` derives `Ord`; callers can reject backends below a required
isolation threshold in a single comparison.

---

## Usage

```rust
use std::path::PathBuf;
use std::time::Duration;
use mofa_kernel::agent::components::sandbox::{
    NetEndpoint, PathPattern, SandboxCapability, SandboxPolicy,
    SandboxResourceLimits,
};

let policy = SandboxPolicy::builder()
    .allow(SandboxCapability::FsRead)
    .allow_fs(PathPattern::Prefix(PathBuf::from("/tmp/tool-scratch")))
    .allow(SandboxCapability::Net)
    .allow_net(NetEndpoint::HostPort {
        host: "api.openai.com".into(),
        port: 443,
    })
    .resource_limits(SandboxResourceLimits {
        wall_timeout: Some(Duration::from_secs(10)),
        memory_limit_bytes: Some(128 * 1024 * 1024),
        ..Default::default()
    })
    .build()
    .unwrap();
```

---

## Observability

Every sandboxed call yields `SandboxExecutionStats`:

```mermaid
classDiagram
    class SandboxExecutionStats {
        +wall_time_ms: Option~u64~
        +cpu_time_ms: Option~u64~
        +peak_memory_bytes: Option~u64~
        +input_bytes: Option~u64~
        +output_bytes: Option~u64~
        +denials: u32
    }
    class SandboxObserver {
        <<trait>>
        +before(backend, tier, request) async
        +after(backend, tier, request, decision, stats) async
    }
    SandboxObserver ..> SandboxExecutionStats
```

`SandboxObserver` is an async hook fired before and after every execution,
suitable for audit-log append, Prometheus metric emission, and policy-drift
detection.

---

## Status

- Kernel trait contracts and policy model — this change
- Foundation backends (`NullSandbox`, `ProcessSandbox`, `WasmtimeSandbox`),
  `SandboxedTool<T>` wrapper, integration tests, runnable example —
  follow-up
- Prometheus sandbox metrics exporter — follow-up

---

## References

- `mofa-kernel::agent::components::sandbox`
- OWASP Tool Sandbox design pattern
- Capability-based security (principle of least authority)
