# Foundation API Reference

The foundation layer (`mofa-foundation`) provides concrete implementations and business logic.

## Modules

### llm
LLM client and provider implementations.

- `LLMClient` — Unified LLM client
- `LLMProvider` — Provider trait
- `OpenAIProvider` — OpenAI implementation
- `AnthropicProvider` — Anthropic implementation

### react
ReAct agent pattern implementation.

- `ReActAgent` — ReAct agent
- `ReActBuilder` — Builder for ReAct agents

### secretary
Secretary agent pattern for human-in-the-loop workflows.

- `SecretaryAgent` — Secretary agent
- `SecretaryConfig` — Configuration

### persistence
Persistence layer for state and session management.

- `PersistencePlugin` — Persistence plugin
- `PostgresStore` — PostgreSQL backend
- `SqliteStore` — SQLite backend

### coordination
Multi-agent coordination patterns.

- `Sequential` — Sequential pipeline
- `Parallel` — Parallel execution
- `Consensus` — Consensus pattern
- `Debate` — Debate pattern

## Feature Flags

| Flag | Description |
|------|-------------|
| `openai` | OpenAI provider |
| `anthropic` | Anthropic provider |
| `persistence` | Persistence layer |

## See Also

- [LLM Providers Guide](../../guides/llm-providers.md) — LLM configuration
- [Persistence Guide](../../guides/persistence.md) — Persistence setup
