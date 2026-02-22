# mofa-foundation

The business layer providing concrete implementations and integrations.

## Purpose

`mofa-foundation` provides:
- LLM integration (OpenAI, Anthropic)
- Agent patterns (ReAct, Secretary)
- Persistence layer
- Workflow orchestration
- Collaboration protocols

## Key Modules

| Module | Description |
|--------|-------------|
| `llm` | LLM client and providers |
| `react` | ReAct agent pattern |
| `secretary` | Secretary agent pattern |
| `persistence` | Storage backends |
| `workflow` | Workflow orchestration |
| `coordination` | Multi-agent coordination |

## Usage

```rust
use mofa_foundation::llm::{LLMClient, openai_from_env};

let client = LLMClient::new(Arc::new(openai_from_env()?));
let response = client.ask("Hello").await?;
```

## Feature Flags

| Flag | Description |
|------|-------------|
| `openai` | OpenAI provider |
| `anthropic` | Anthropic provider |
| `persistence` | Persistence layer |

## Architecture Rules

- ✅ Import traits from kernel
- ✅ Provide implementations
- ❌ NEVER re-define kernel traits

## See Also

- [LLM Providers](../guides/llm-providers.md) — LLM configuration
- [Persistence](../guides/persistence.md) — Persistence guide
