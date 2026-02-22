# Agent Trait

The `MoFAAgent` trait is the core interface for all agents.

## Definition

```rust
#[async_trait]
pub trait MoFAAgent: Send + Sync {
    /// Unique identifier for this agent
    fn id(&self) -> &str;

    /// Human-readable name
    fn name(&self) -> &str;

    /// Agent capabilities and metadata
    fn capabilities(&self) -> &AgentCapabilities;

    /// Current lifecycle state
    fn state(&self) -> AgentState;

    /// Initialize the agent
    async fn initialize(&mut self, ctx: &AgentContext) -> AgentResult<()>;

    /// Execute the main agent logic
    async fn execute(
        &mut self,
        input: AgentInput,
        ctx: &AgentContext,
    ) -> AgentResult<AgentOutput>;

    /// Shutdown the agent
    async fn shutdown(&mut self) -> AgentResult<()>;

    // Optional lifecycle hooks
    async fn pause(&mut self) -> AgentResult<()> { Ok(()) }
    async fn resume(&mut self) -> AgentResult<()> { Ok(()) }
}
```

## Lifecycle

```
Created → initialize() → Ready → execute() → Executing → Ready
                                      ↓
                               shutdown() → Shutdown
```

## Example Implementation

```rust
use mofa_sdk::kernel::agent::prelude::*;

struct MyAgent {
    id: String,
    name: String,
    capabilities: AgentCapabilities,
    state: AgentState,
}

#[async_trait]
impl MoFAAgent for MyAgent {
    fn id(&self) -> &str { &self.id }
    fn name(&self) -> &str { &self.name }
    fn capabilities(&self) -> &AgentCapabilities { &self.capabilities }
    fn state(&self) -> AgentState { self.state.clone() }

    async fn initialize(&mut self, _ctx: &AgentContext) -> AgentResult<()> {
        self.state = AgentState::Ready;
        Ok(())
    }

    async fn execute(
        &mut self,
        input: AgentInput,
        _ctx: &AgentContext,
    ) -> AgentResult<AgentOutput> {
        self.state = AgentState::Executing;
        let result = format!("Processed: {}", input.to_text());
        self.state = AgentState::Ready;
        Ok(AgentOutput::text(result))
    }

    async fn shutdown(&mut self) -> AgentResult<()> {
        self.state = AgentState::Shutdown;
        Ok(())
    }
}
```

## See Also

- [AgentContext](context.md) — Execution context
- [AgentInput/Output](types.md) — Input and output types
- [Agents Concept](../../concepts/agents.md) — Agent overview
