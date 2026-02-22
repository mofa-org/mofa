# Agent Registry

Registry for managing and discovering agents.

## Overview

`AgentRegistry` provides:
- Agent registration and deregistration
- Agent discovery by capabilities
- Agent lifecycle management

## Definition

```rust
pub trait AgentRegistry: Send + Sync {
    async fn register(&mut self, agent: Box<dyn MoFAAgent>) -> AgentResult<()>;
    async fn unregister(&mut self, id: &str) -> AgentResult<()>;
    async fn get(&self, id: &str) -> Option<&dyn MoFAAgent>;
    async fn find_by_capability(&self, tag: &str) -> Vec<&dyn MoFAAgent>;
    async fn list_all(&self) -> Vec<&dyn MoFAAgent>;
}
```

## Usage

```rust
use mofa_sdk::runtime::SimpleRegistry;

let mut registry = SimpleRegistry::new();

// Register agents
registry.register(Box::new(ResearcherAgent::new())).await?;
registry.register(Box::new(WriterAgent::new())).await?;
registry.register(Box::new(EditorAgent::new())).await?;

// Find by capability
let research_agents = registry.find_by_capability("research").await;

// Get by ID
let agent = registry.get("researcher-1").await;

// List all
for agent in registry.list_all().await {
    println!("{}", agent.name());
}
```

## SimpleRegistry

The default in-memory implementation:

```rust
pub struct SimpleRegistry {
    agents: HashMap<String, Box<dyn MoFAAgent>>,
}
```

## Discovery

Find agents by tags or capabilities:

```rust
// Find by single tag
let agents = registry.find_by_capability("llm").await;

// Find by multiple tags
let agents = registry.find_by_tags(&["llm", "qa"]).await;

// Find by input type
let agents = registry.find_by_input_type(InputType::Text).await;
```

## See Also

- [AgentRunner](runner.md) — Agent execution
- [Agents](../../concepts/agents.md) — Agent concepts
