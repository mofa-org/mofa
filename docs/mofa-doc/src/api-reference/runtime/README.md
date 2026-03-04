# Runtime API Reference

The runtime layer (`mofa-runtime`) manages agent lifecycle and execution.

## Core Components

- [AgentRunner](runner.md) — Execute agents with lifecycle management
- [AgentBuilder](builder.md) — Build agents step by step
- [SimpleRuntime](runtime.md) — Multi-agent runtime

## Overview

```rust
use mofa_sdk::runtime::AgentRunner;
use mofa_sdk::kernel::{AgentInput, AgentContext};

// Create runner with an agent
let mut runner = AgentRunner::new(my_agent).await?;

// Execute
let output = runner.execute(AgentInput::text("Hello")).await?;

// Shutdown
runner.shutdown().await?;
```

## Modules

### runner
Agent execution with lifecycle management.

### builder
Builder pattern for constructing agents.

### registry
Agent registration and discovery.

### coordination
Multi-agent coordination patterns.

## Feature Flags

| Flag | Description |
|------|-------------|
| `dora` | Dora-rs distributed runtime |
| `monitoring` | Built-in monitoring |

## See Also

- [Architecture](../../concepts/architecture.md) — Runtime layer
- [Agents](../../concepts/agents.md) — Agent lifecycle
