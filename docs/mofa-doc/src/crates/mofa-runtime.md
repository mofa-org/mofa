# mofa-runtime

The runtime layer managing agent lifecycle and execution.

## Purpose

`mofa-runtime` provides:
- `AgentRunner` for execution management
- `AgentBuilder` for constructing agents
- `SimpleRuntime` for multi-agent coordination
- Message bus and event routing
- Plugin management

## Key Components

| Component | Description |
|-----------|-------------|
| `AgentRunner` | Execute agents with lifecycle |
| `AgentBuilder` | Build agents step-by-step |
| `SimpleRuntime` | Multi-agent runtime |
| `PluginManager` | Manage plugins |

## Usage

```rust
use mofa_runtime::AgentRunner;
use mofa_kernel::{AgentInput, AgentContext};

let mut runner = AgentRunner::new(my_agent).await?;
let output = runner.execute(AgentInput::text("Hello")).await?;
runner.shutdown().await?;
```

## Feature Flags

| Flag | Description |
|------|-------------|
| `dora` | Dora-rs distributed runtime |
| `monitoring` | Built-in monitoring |

## See Also

- [Agents](../concepts/agents.md) — Agent concepts
- [Workflows](../concepts/workflows.md) — Workflow orchestration
