# mofa-sdk

The unified SDK providing the main API surface for users.

## Purpose

`mofa-sdk` provides:
- Re-exports from all layers
- Cross-language bindings (UniFFI, PyO3)
- Convenient builder patterns
- Secretary agent mode

## Module Organization

```rust
use mofa_sdk::{
    kernel,   // Core abstractions
    runtime,  // Runtime components
    llm,      // LLM integration
    plugins,  // Plugin system
};
```

## Usage

```rust
use mofa_sdk::kernel::prelude::*;
use mofa_sdk::llm::{LLMClient, openai_from_env};
use mofa_sdk::runtime::AgentRunner;

let client = LLMClient::new(Arc::new(openai_from_env()?));
let agent = MyAgent::new(client);
let mut runner = AgentRunner::new(agent).await?;
```

## Feature Flags

| Flag | Description |
|------|-------------|
| `openai` | OpenAI provider |
| `anthropic` | Anthropic provider |
| `uniffi` | Cross-language bindings |
| `python` | Native Python bindings |

## See Also

- [Getting Started](../getting-started/installation.md) — Quick start
- [API Reference](../api-reference/kernel/README.md) — API docs
