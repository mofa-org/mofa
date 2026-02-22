# Kernel API Reference

The kernel layer (`mofa-kernel`) provides core abstractions and types.

## Modules

### agent
Core agent traits and types.

- [`MoFAAgent`](agent.md) — Core agent trait
- [`AgentContext`](context.md) — Execution context
- [`AgentInput` / `AgentOutput`](types.md) — Input/output types

### components
Agent components like tools and memory.

- [`Tool`](./components/tool.md) — Tool trait
- [`Memory`](./components/memory.md) — Memory trait
- [`Reasoner`](./components/reasoner.md) — Reasoning interface

### plugin
Plugin system interfaces.

- [`AgentPlugin`](./plugin.md) — Plugin trait

## Core Types

```rust
// Agent states
pub enum AgentState {
    Created,
    Ready,
    Executing,
    Paused,
    Error,
    Shutdown,
}

// Capabilities
pub struct AgentCapabilities {
    pub tags: Vec<String>,
    pub input_type: InputType,
    pub output_type: OutputType,
    pub max_concurrency: usize,
}

// Error handling
pub type AgentResult<T> = Result<T, AgentError>;

pub enum AgentError {
    InitializationFailed(String),
    ExecutionFailed(String),
    InvalidInput(String),
    ToolNotFound(String),
    Timeout,
    // ...
}
```

## Feature Flags

The kernel has no optional features—it always provides the minimal core.

## See Also

- [Architecture](../../concepts/architecture.md) — Architecture overview
- [Microkernel Design](../../concepts/microkernel.md) — Design principles
