# mofa-kernel

The microkernel core providing minimal abstractions and types.

## Purpose

`mofa-kernel` provides:
- Core trait definitions (`MoFAAgent`, `Tool`, `Memory`)
- Base types (`AgentInput`, `AgentOutput`, `AgentState`)
- Plugin interfaces
- Event bus primitives

**Important**: This crate contains NO implementations, only interfaces.

## Key Traits

| Trait | Description |
|-------|-------------|
| `MoFAAgent` | Core agent interface |
| `Tool` | Tool interface for function calling |
| `Memory` | Memory/storage interface |
| `AgentPlugin` | Plugin interface |

## Usage

```rust
use mofa_kernel::agent::prelude::*;

struct MyAgent { /* ... */ }

#[async_trait]
impl MoFAAgent for MyAgent {
    // Implementation
}
```

## Architecture Rules

- ✅ Define traits here
- ✅ Define core types here
- ❌ NO implementations (except test code)
- ❌ NO business logic

## Feature Flags

None - kernel is always minimal.

## See Also

- [Architecture](../concepts/architecture.md) — Architecture overview
- [Microkernel Design](../concepts/microkernel.md) — Design principles
