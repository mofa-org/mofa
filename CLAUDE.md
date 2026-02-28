# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

MoFA (Modular Framework for Agents) is a production-grade AI agent framework built in Rust, designed for extreme performance, unlimited extensibility, and runtime programmability. It implements a **microkernel + dual-layer plugin system** architecture.

**Key Features:**
- Rust core with UniFFI for multi-language bindings (Python, Java, Swift, Kotlin, Go)
- Dual-layer plugins: compile-time (Rust/WASM) for performance + runtime (Rhai scripts) for flexibility
- Multi-agent coordination patterns (chain, parallel, debate, supervision, MapReduce, routing, aggregation)
- Secretary agent mode for human-in-the-loop workflows
- Distributed dataflow support via Dora-rs (optional)
- Actor-based concurrency using Ractor

## Common Commands

```bash
# Build the entire workspace
cargo build
cargo build --release

# Build specific crate
cargo build -p mofa-sdk
cargo build -p mofa-cli

# Run tests
cargo test
cargo test -p mofa-sdk

# Run specific test
cargo test -p mofa-sdk -- test_name

# Run CLI tool
cargo run -p mofa-cli -- mofa --help

# Run examples
cargo run -p mofa-cli -- mofa new my_agent
cd examples/react_agent && cargo run
cd examples/secretary_agent && cargo run

# Check code (fast compilation check)
cargo check

# Format code
cargo fmt

# Run linter
cargo clippy
```

## Workspace Structure

```
mofa/
├── Cargo.toml              # Workspace root
├── crates/
│   ├── mofa-kernel/        # Microkernel core (lifecycle, metadata, communication)
│   ├── mofa-foundation/    # Foundation layer (LLM, agents, persistence)
│   ├── mofa-runtime/       # Runtime system (message bus, registry, event loop)
│   ├── mofa-plugins/       # Plugin system (dual-layer architecture)
│   ├── mofa-cli/           # CLI tool (`mofa` command)
│   ├── mofa-sdk/           # Main SDK - standard API surface
│   ├── mofa-ffi/           # FFI bindings (UniFFI for Python, Java, Go, Kotlin, Swift)
│   ├── mofa-macros/        # Procedural macros
│   ├── mofa-monitoring/    # Monitoring and observability
│   └── mofa-extra/         # Additional utilities
├── examples/               # Usage examples (27+ examples)
└── docs/                   # Documentation
```

## Architecture Overview

### Microkernel + Dual-Layer Plugin System

MoFA uses a layered microkernel architecture:

1. **Microkernel (`mofa-kernel`)**: Lightweight core with lifecycle management, metadata system, communication bus, and task scheduling
2. **Compile-time Plugin Layer**: Rust/WASM plugins for performance-critical paths (LLM inference, data processing, native system integration)
3. **Runtime Plugin Layer**: Rhai scripts for dynamic business logic, hot-reloadable rules, workflow orchestration
4. **Business Layer**: User-defined agents, workflows, and rules

### Key Crates

- **`mofa-sdk`**: Main entry point - high-level standard API, multi-language bindings, secretary agent mode
- **`mofa-runtime`**: Message bus, agent registry, event loop, health checks, state management
- **`mofa-foundation`**: LLM integration (OpenAI provider), agent abstractions, persistence layer (PostgreSQL/MySQL/SQLite)
- **`mofa-plugins`**: Dual-layer plugin system with Rhai scripting engine integration
- **`mofa-kernel`**: Core runtime with metadata, lifecycle, and communication primitives

### Multi-Agent Coordination Patterns

The framework supports 7 LLM-driven collaboration modes:
- **Request-Response**: One-to-one deterministic tasks with synchronous replies
- **Publish-Subscribe**: One-to-many broadcast tasks with multiple receivers
- **Consensus**: Multi-round negotiation and voting for decision-making
- **Debate**: Multi-agent alternating discussion for quality improvement
- **Parallel**: Simultaneous execution with result aggregation
- **Sequential**: Pipeline execution (output of one agent becomes input of next)
- **Custom**: User-defined modes interpreted by the LLM

### Secretary Agent Pattern

Human-in-the-loop workflow management with 5 phases:
1. **Receive ideas** → Record todos
2. **Clarify requirements** → Project documents
3. **Schedule dispatch** → Call execution agents
4. **Monitor feedback** → Push key decisions to humans
5. **Acceptance report** → Update todos

## Development Notes

### Code and Documentation Language

**English is the primary language for all code comments and documentation.**
- Use English for inline comments, doc comments (`///`, `//!`), and README files
- Variable, function, and type names should be in English
- Commit messages should be written in English
- This ensures consistency and accessibility for international contributors

### Feature Flags

The workspace uses feature flags extensively:
- `uniffi`: Cross-language bindings (Python, Java, Swift, Kotlin, Go)
- `openai`: OpenAI provider support
- `dora`: Dora-rs distributed runtime support
- `persistence-*`: Database backend selection (postgres, mysql, sqlite)
- `python`: Native Python bindings via PyO3

### Dependencies

Key dependencies:
- `tokio`: Async runtime
- `ractor`: Actor framework for ReAct agents
- `rhai`: Embedded scripting engine for runtime plugins
- `uniffi`: Multi-language bindings generator
- `sqlx`: Database access
- `opentelemetry`: Distributed tracing
- `serde`/`serde_json`: Serialization

### Plugin Development

- Compile-time plugins use Rust traits for zero-cost abstractions
- Runtime plugins use Rhai scripting with built-in JSON processing
- Both layers can interoperate seamlessly

### Testing

Run tests for specific crates:
- `cargo test -p mofa-sdk`: Test SDK functionality
- `cargo test -p mofa-runtime`: Test runtime systems
- `cargo test -p mofa-plugins`: Test plugin system

### Examples Directory

The `examples/` directory contains 27+ examples demonstrating various features:
- `react_agent/`: Basic ReAct pattern agent
- `secretary_agent/`: Secretary agent with human-in-the-loop
- `hitl_secretary/`: Human-in-the-loop secretary variant
- `multi_agent_coordination/`: Various coordination patterns
- `adaptive_collaboration_agent/`: Adaptive collaboration demo
- `chat_stream/`: Streaming chat example
- `rhai_scripting/`: Runtime scripting
- `rhai_hot_reload/`: Hot-reloadable Rhai scripts
- `workflow_orchestration/`: Workflow builder
- `workflow_dsl/`: Workflow DSL usage
- `wasm_plugin/`: WASM plugin development
- `monitoring_dashboard/`: Observability features
- `plugin_demo/`, `plugin_system/`: Plugin development
- `python_bindings/`, `java_bindings/`, `go_bindings/`: FFI examples
- `tool_routing/`, `skills/`: Tool and skill management
- `streaming_persistence/`, `streaming_manual_persistence/`: Persistence examples
- `financial_compliance_agent/`, `medical_diagnosis_agent/`: Domain-specific agents

Review these when implementing new features to understand existing patterns.

---

## Rust Project Development Standards

Based on issues discovered during code reviews, the following general development standards have been compiled:

### I. Error Handling Standards

#### 1. Unified Error System

- **MUST** define a unified error type in the crate root (e.g., `KernelError`)
- **MUST** establish a clear error hierarchy where module errors can be unified through the `From` trait
- **MUST NOT** use `anyhow::Result` as a public API return type in library code; use `thiserror` to define typed errors
- **MUST NOT** implement blanket `From<anyhow::Error>` for error types, as this erases structured error information

#### 2. Error Type Design

```rust
// Recommended: Use thiserror to define clear error enums
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum KernelError {
    #[error("Agent error: {0}")]
    Agent(#[from] AgentError),
    #[error("Config error: {0}")]
    Config(#[from] ConfigError),
    // ...
}
```

### II. Type Design and API Stability

#### 1. Enum Extensibility

- **MUST** add the `#[non_exhaustive]` attribute to public enums to ensure backward compatibility

```rust
#[derive(Debug, Clone)]
#[non_exhaustive]
pub enum AgentState {
    Idle,
    Running,
    // New variants can be safely added in the future
}
```

#### 2. Derive Trait Standards

- Comparable/testable types **MUST** derive `PartialEq`, `Eq`
- Debug output types **MUST** derive `Debug`; for fields that cannot be auto-derived, implement manually
- Serializable types **MUST** derive `Clone` (unless there's a special reason not to)

### III. Naming and Module Design

#### 1. Naming Uniqueness

- **MUST NOT** define types with the same name representing different concepts within the same crate
- Checklist: `AgentConfig`, `AgentEvent`, `TaskPriority`, and other core type names

#### 2. Module Export Control

- **MUST** use `pub(crate)` to limit internal module visibility
- **MUST** carefully design the public API surface through `lib.rs` or `prelude`
- **MUST NOT** directly `pub mod` export all modules

```rust
// Recommended lib.rs structure
pub mod error;
pub mod agent;
pub use error::KernelError;
pub use agent::{Agent, AgentContext};
mod internal; // Internal implementation
```

#### 3. Prelude Design

- **SHOULD** provide a crate-level prelude module that aggregates commonly used types

```rust
// src/prelude.rs
pub use crate::error::KernelError;
pub use crate::agent::{Agent, AgentContext, AgentState};
// ...
```

### IV. Performance and Dependencies Management

#### 1. Async Features

- In Rust 1.75+ environments, **SHOULD** use native `async fn in trait` instead of `#[async_trait]`
- Only use `async` on methods that genuinely require async; synchronous operations should not be marked as async

#### 2. Avoid Repeated Computation

- Objects with high compilation costs like regular expressions **MUST** be cached using `LazyLock` or `OnceLock`

```rust
use std::sync::LazyLock;
static ENV_VAR_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\$\{([^}]+)\}").unwrap()
});
```

#### 3. Timestamp Handling

- Timestamp generation logic **MUST** be abstracted into a single utility function
- **SHOULD** provide an injectable clock abstraction for testing

```rust
pub trait Clock: Send + Sync {
    fn now_millis(&self) -> u64;
}

pub struct SystemClock;
impl Clock for SystemClock {
    fn now_millis(&self) -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64
    }
}
```

#### 4. Avoid Reinventing the Wheel

- **MUST NOT** hand-write logic for Base64, encryption algorithms, etc. that have mature implementations
- Prioritize using widely validated community crates

### V. Type Safety

#### 1. Reduce Dynamic Type Usage

- **MUST NOT** abuse `serde_json::Value` in scenarios where generic constraints can be used
- **AVOID** using `Box<dyn Any + Send + Sync>` as generic storage; prefer generics or trait objects with specific traits

#### 2. Mutability and Interface Consistency

- The choice between `&self` and `&mut self` in trait method signatures **MUST** be consistent
- If internal state modification through `&self` is needed (e.g., `Arc<RwLock<_>>`), document side effects clearly

### VI. Interface Consistency

#### 1. Parameter Type Conventions

- Constructor parameter types **SHOULD** be unified: prefer `impl Into<String>` or `&str`

```rust
// Recommended
pub fn new(id: impl Into<String>) -> Self { ... }
// Avoid
pub fn new(id: String) -> Self { ... }
```

#### 2. Builder Pattern Validation

- Builder methods **MUST** validate invalid input or return `Result`

```rust
pub fn with_weight(mut self, weight: f64) -> Result<Self, &'static str> {
    if weight < 0.0 {
        return Err("Weight must be non-negative");
    }
    self.weight = Some(weight);
    Ok(self)
}
```

#### 3. Naming Conventions

- **MUST NOT** create custom method names that conflict with standard trait method names (e.g., `to_string_output` vs `to_string`)

### VII. Code Correctness

#### 1. Manual Ord/Eq Implementation

- **MUST** write complete tests covering all branches for manually implemented `Ord` trait
- Recommend using `derive` or simplified implementations based on discriminants

#### 2. Type Conversion Safety

- Numeric type conversions **MUST** explicitly handle potential overflow

```rust
// Avoid
let ts = as_millis() as u64;
// Recommended
let ts = u64::try_from(as_millis()).unwrap_or(u64::MAX);
```

### VIII. Serialization and Compatibility

#### 1. Message Protocol Versioning

- Binary serialization **MUST** include version identifiers

```rust
#[derive(Serialize, Deserialize)]
struct MessageEnvelope {
    version: u8,
    payload: Vec<u8>,
}
```

#### 2. Serialization Abstraction

- Message buses **SHOULD** support pluggable serialization backends

```rust
pub trait Serializer: Send + Sync {
    fn serialize<T: Serialize>(&self, value: &T) -> Result<Vec<u8>>;
    fn deserialize<T: DeserializeOwned>(&self, data: &[u8]) -> Result<T>;
}
```

### IX. Testing Standards

#### 1. Test Coverage

- **MUST** include: boundary values, null values, invalid input, concurrent scenarios
- **MUST NOT** only test the happy path

#### 2. Unit Tests and Integration Tests

- **MUST** write unit tests for core logic
- **SHOULD** write integration tests for inter-module interactions

#### 3. Testability Design

- External dependencies (clock, random numbers, network) **MUST** be injectable through traits for mock implementations

### X. Feature Isolation

#### 1. Feature Flag Standards

- Dependencies behind feature gates **MUST** be marked with `optional = true` in `Cargo.toml`
- **MUST NOT** feature gate partial code while the dependency is still compiled unconditionally

```toml
[dependencies]
config = { version = "0.14", optional = true }

[features]
default = []
config-loader = ["dep:config"]
```

### Development Standards Checklist

| Check Item | Requirement | Status |
|------------|-------------|--------|
| Public enums have `#[non_exhaustive]` | Must | ☐ |
| Public error types are unified | Must | ☐ |
| No types with same name but different meanings | Forbidden | ☐ |
| Traits have unnecessary async usage | Check | ☐ |
| Numeric conversions have overflow risk | Check | ☐ |
| Time-related code is testable | Must | ☐ |
| Builders have input validation | Must | ☐ |
| Regex etc. use caching | Must | ☐ |
| Integration tests exist | Should | ☐ |
| Error path test coverage | Must | ☐ |

---

## MoFA Microkernel Architecture Standards

### Architecture Layering

MoFA follows a strict microkernel architecture with clear separation of concerns:

```
┌─────────────────────────────────────────────────────────────┐
│                    mofa-sdk (Standard API)                  │
│  - External standard interface                               │
│  - Re-exports core types from kernel and foundation          │
└─────────────────────────────────────────────────────────────┘
                            ↓
┌─────────────────────────────────────────────────────────────┐
│              mofa-runtime (Execution Lifecycle)              │
│  - AgentRegistry, EventLoop, PluginManager                  │
│  - Dynamic loading and plugin management                      │
└─────────────────────────────────────────────────────────────┘
                            ↓
┌─────────────────────────────────────────────────────────────┐
│            mofa-foundation (Business Logic)                  │
│  - ✅ Concrete implementations (InMemoryStorage, SimpleToolRegistry) |
│  - ✅ Extended types (RichAgentContext, business-specific data)  |
│  - ❌ FORBIDDEN: Re-defining kernel traits                          │
│  - ✅ ALLOWED: Importing and extending kernel traits                   │
└─────────────────────────────────────────────────────────────┘
                            ↓
┌─────────────────────────────────────────────────────────────┐
│              mofa-kernel (Microkernel Core)                  │
│  - ✅ Trait definitions (Tool, Memory, Reasoner, etc.)       │
│  - ✅ Core data types (AgentInput, AgentOutput, AgentState)  │
│  - ✅ Base abstractions (MoFAAgent, AgentPlugin)             │
│  - ❌ FORBIDDEN: Concrete implementations (except test code)   │
│  - ❌ FORBIDDEN: Business logic                                │
└─────────────────────────────────────────────────────────────┘
                            ↓
┌─────────────────────────────────────────────────────────────┐
│            mofa-plugins (Plugin Layer)                       │
│  - Plugin adapters (ToolPluginAdapter)                       │
│  - Concrete plugin implementations                            │
└─────────────────────────────────────────────────────────────┘
```

### Core Rules

#### Rule 1: Trait Definition Location
- ✅ **Kernel Layer**: Define ALL core trait interfaces
- ❌ **Foundation Layer**: NEVER re-define the same trait from kernel
- ✅ **Foundation Layer**: CAN import traits from kernel and add extension methods

#### Rule 2: Implementation Location
- ✅ **Foundation Layer**: Provide ALL concrete implementations
- ❌ **Kernel Layer**: NO concrete implementations (test code excepted)
- ✅ **Plugins Layer**: Provide optional advanced implementations

#### Rule 3: Type Exports
- ✅ **Kernel**: Export only types it defines
- ✅ **Foundation**: Export only types it implements, NOT re-export kernel traits
- ✅ **SDK**: Standard re-export of user-facing APIs

#### Rule 4: Data Types
- ✅ **Kernel Layer**: Base data types (AgentInput, AgentOutput, AgentState, ToolInput, ToolResult)
- ✅ **Foundation Layer**: Business-specific data types (Session, PromptContext, ComponentOutput)
- ⚠️ **Boundary**: If a type is part of a trait definition, put it in kernel; if business-specific, put it in foundation

#### Rule 5: Dependency Direction
```
Foundation → Kernel (ALLOWED)
Plugins → Kernel (ALLOWED)
Plugins → Foundation (ALLOWED)
Kernel → Foundation (FORBIDDEN! Creates circular dependency)
```

### Code Checklist

#### Kernel Layer Checklist
- [ ] Is this a trait definition? ✅ Otherwise it shouldn't be here
- [ ] Is this a core data type? ✅ AgentInput/Output/State, ToolInput/Result, etc.
- [ ] Is this a base type? ✅ Interfaces, primitives
- [ ] Does it contain concrete implementations? ❌ Move to foundation

#### Foundation Layer Checklist
- [ ] Does it re-define a kernel trait? ❌ Import from kernel instead
- [ ] Is this a concrete implementation? ✅ Correct location
- [ ] Does it depend on kernel types? ✅ Allowed: `use mofa_kernel::...`
- [ ] Is it depended on by kernel? ❌ Creates circular dependency

#### Type Export Checklist
- [ ] Does foundation re-export kernel traits? ❌ Remove duplicate exports
- [ ] Can users clearly tell which layer a type comes from? ✅ Should be clear
- [ ] Are there naming conflicts? ❌ Should be avoided

### Common Anti-Patterns

#### ❌ Anti-Pattern 1: Foundation Re-defining Kernel Trait

```rust
// crates/mofa-foundation/src/agent/components/tool.rs
// ❌ WRONG: Re-defining kernel trait in foundation
#[async_trait]
pub trait Tool: Send + Sync {
    fn name(&self) -> &str;
    // ...
}
```

**Correct Approach**:
```rust
// ✅ CORRECT: Import from kernel
pub use mofa_kernel::agent::components::tool::Tool;

// If you need to extend, define a wrapper
pub struct FoundationTool {
    inner: Arc<dyn Tool>,
    extra_field: String,
}
```

#### ❌ Anti-Pattern 2: Kernel Containing Concrete Implementation

```rust
// crates/mofa-kernel/src/agent/components/tool.rs
// ❌ WRONG: Kernel should not contain concrete implementations
pub struct SimpleToolRegistry {
    tools: HashMap<String, Arc<dyn Tool>>,
}

#[async_trait]
impl ToolRegistry for SimpleToolRegistry {
    // Implementation...
}
```

**Correct Approach**:
```rust
// ✅ CORRECT: Only define trait in kernel
#[async_trait]
pub trait ToolRegistry: Send + Sync {
    fn register(&mut self, tool: Arc<dyn Tool>) -> AgentResult<()>;
    // ...
}

// Concrete implementation goes in foundation
// crates/mofa-foundation/src/agent/components/tool_registry.rs
pub struct SimpleToolRegistry {
    tools: HashMap<String, Arc<dyn Tool>>,
}

#[async_trait]
impl ToolRegistry for SimpleToolRegistry {
    // Implementation...
}
```

#### ❌ Anti-Pattern 3: Duplicate Exports Causing Type Confusion

```rust
// crates/mofa-foundation/src/agent/mod.rs
// ❌ WRONG: Duplicate exports of kernel types
pub use mofa_kernel::agent::{Tool, ToolRegistry};
pub use components::tool::{Tool, ToolRegistry, SimpleToolRegistry};
```

**Correct Approach**:
```rust
// ✅ CORRECT: Foundation exports only what it implements
pub use components::tool_registry::{SimpleToolRegistry, EchoTool};
pub use components::tool::ToolCategory; // Foundation-specific extension

// Tool and ToolRegistry are exported by kernel, no need to re-export here
```

### Identifying Architecture Violations

When reviewing code, check for these warning signs:

1. **Same trait name in multiple crates**: Indicates duplicate definition
2. **Foundation `pub use` of kernel traits with custom modifications**: Likely should be extension not replacement
3. **Kernel `pub struct` with trait implementations**: Concrete code in wrong layer
4. **Circular dependency warnings in Cargo.toml**: Architecture violation

### Quick Reference

| What | Where | Example |
|-------|-------|---------|
| **Trait definitions** | `mofa-kernel` | `Tool`, `Memory`, `Reasoner`, `Coordinator` |
| **Core data types** | `mofa-kernel` | `AgentInput`, `AgentOutput`, `AgentState` |
| **Base abstractions** | `mofa-kernel` | `MoFAAgent`, `AgentPlugin` |
| **Concrete implementations** | `mofa-foundation` | `SimpleToolRegistry`, `InMemoryStorage` |
| **Business types** | `mofa-foundation` | `Session`, `PromptContext`, `RichAgentContext` |
| **Plugin implementations** | `mofa-plugins` | `ToolPluginAdapter`, `LLMPlugin` |
