# Guides

Practical guides for common tasks and patterns.

## Overview

- **LLM Providers** — Configure different LLM backends
- **Tool Development** — Create custom tools
- **Persistence** — Save and restore agent state
- **Multi-Agent Systems** — Coordinate multiple agents
- **Secretary Agent** — Human-in-the-loop patterns
- **Skills System** — Composable agent capabilities
- **Monitoring & Observability** — Production monitoring

## Common Patterns

### Building a ReAct Agent

```rust
let agent = ReActAgent::builder()
    .with_llm(client)
    .with_tools(vec![tool1, tool2])
    .build();
```

### Multi-Agent Coordination

```rust
let coordinator = SequentialCoordinator::new()
    .add_agent(researcher)
    .add_agent(writer);
```

## Next Steps

Choose a guide based on your use case.
