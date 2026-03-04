# Agent Patterns

Built-in agent patterns for common use cases.

## Overview

MoFA provides several agent patterns:

| Pattern | Use Case |
|---------|----------|
| ReAct | Reasoning + Acting with tools |
| Secretary | Human-in-the-loop coordination |
| Chain-of-Thought | Step-by-step reasoning |
| Router | Route to specialized agents |

## ReAct Pattern

Reasoning and Acting agent that uses tools iteratively.

```rust
use mofa_sdk::react::ReActAgent;

let agent = ReActAgent::builder()
    .with_llm(client)
    .with_tools(vec![
        Arc::new(SearchTool),
        Arc::new(CalculatorTool),
    ])
    .with_max_iterations(5)
    .build();

let output = agent.execute(input, &ctx).await?;
```

### Configuration

```rust
pub struct ReActConfig {
    max_iterations: usize,
    tool_timeout: Duration,
    reasoning_template: String,
}
```

## Secretary Pattern

Human-in-the-loop coordination agent.

```rust
use mofa_sdk::secretary::SecretaryAgent;

let agent = SecretaryAgent::builder()
    .with_llm(client)
    .with_human_feedback(true)
    .with_delegation_targets(vec!["researcher", "writer"])
    .build();
```

### Phases

1. Receive Ideas → Record todos
2. Clarify Requirements → Generate documents
3. Schedule Dispatch → Call agents
4. Monitor Feedback → Push decisions to humans
5. Acceptance Report → Update status

## Chain-of-Thought

Step-by-step reasoning without tools.

```rust
use mofa_sdk::patterns::ChainOfThought;

let agent = ChainOfThought::builder()
    .with_llm(client)
    .with_steps(5)
    .build();
```

## Router Pattern

Route requests to specialized agents.

```rust
use mofa_sdk::patterns::Router;

let router = Router::builder()
    .with_classifier(classifier_agent)
    .with_route("technical", tech_agent)
    .with_route("billing", billing_agent)
    .with_default(general_agent)
    .build();

let output = router.execute(input, &ctx).await?;
```

## Custom Patterns

Implement your own pattern:

```rust
use mofa_sdk::kernel::prelude::*;

struct MyPattern {
    agents: Vec<Box<dyn MoFAAgent>>,
}

#[async_trait]
impl MoFAAgent for MyPattern {
    async fn execute(&mut self, input: AgentInput, ctx: &AgentContext) -> AgentResult<AgentOutput> {
        // Your pattern logic
    }
}
```

## See Also

- [Secretary Agent Guide](../../guides/secretary-agent.md) — Secretary details
- [Workflows](../../concepts/workflows.md) — Workflow orchestration
