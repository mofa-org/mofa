# mofa-macros

Procedural macros for MoFA.

## Purpose

`mofa-macros` provides:
- Derive macros for common traits
- Attribute macros for configuration
- Code generation helpers

## Available Macros

### #[agent]

Auto-implement common agent functionality:

```rust
use mofa_macros::agent;

#[agent(tags = ["llm", "qa"])]
struct MyAgent {
    llm: LLMClient,
}

// Automatically implements:
// - id(), name(), capabilities()
// - Default state management
```

### #[tool]

Define tools with less boilerplate:

```rust
use mofa_macros::tool;

#[tool(name = "calculator", description = "Performs arithmetic")]
fn calculate(operation: String, a: f64, b: f64) -> f64 {
    match operation.as_str() {
        "add" => a + b,
        "subtract" => a - b,
        _ => panic!("Unknown operation"),
    }
}
```

## Usage

Add to `Cargo.toml`:

```toml
[dependencies]
mofa-macros = "0.1"
```

## See Also

- [Tool Development](../guides/tool-development.md) — Tool guide
- [Agents](../concepts/agents.md) — Agent concepts
