# Multi-Agent Coordination

Examples of multi-agent coordination patterns.

## Sequential Pipeline

Execute agents in sequence, passing output from one to the next.

**Location:** `examples/multi_agent_coordination/`

```rust
use mofa_sdk::coordination::Sequential;
use mofa_sdk::runtime::AgentRunner;

#[tokio::main]
async fn main() -> Result<()> {
    // Create agents
    let researcher = ResearcherAgent::new();
    let analyst = AnalystAgent::new();
    let writer = WriterAgent::new();

    // Build pipeline
    let pipeline = Sequential::new()
        .add_step(researcher)  // Step 1: Research
        .add_step(analyst)     // Step 2: Analyze
        .add_step(writer);     // Step 3: Write

    // Execute pipeline
    let input = AgentInput::text("Write a report on AI trends");
    let output = pipeline.execute(input).await?;

    println!("{}", output.as_text().unwrap());
    Ok(())
}
```

## Parallel Execution

Execute multiple agents simultaneously.

```rust
use mofa_sdk::coordination::Parallel;

#[tokio::main]
async fn main() -> Result<()> {
    let parallel = Parallel::new()
        .with_agents(vec![
            FactCheckerAgent::new(),
            StyleCheckerAgent::new(),
            GrammarCheckerAgent::new(),
        ])
        .with_aggregation(Aggregation::MergeAll);

    let input = AgentInput::text("Check this article...");
    let results = parallel.execute(input).await?;

    // Results from all agents
    Ok(())
}
```

## Consensus Pattern

Multiple agents negotiate to reach agreement.

```rust
use mofa_sdk::coordination::Consensus;

#[tokio::main]
async fn main() -> Result<()> {
    let consensus = Consensus::new()
        .with_agents(vec![
            ExpertA::new(),
            ExpertB::new(),
            ExpertC::new(),
        ])
        .with_threshold(0.6)  // 60% agreement
        .with_max_rounds(5);

    let proposal = AgentInput::text("Should we use microservices?");
    let decision = consensus.decide(&proposal).await?;

    println!("Decision: {:?}", decision);
    Ok(())
}
```

## Debate Pattern

Agents debate a topic with a judge.

```rust
use mofa_sdk::coordination::Debate;

#[tokio::main]
async fn main() -> Result<()> {
    let debate = Debate::new()
        .with_proposer(ProAgent::new())
        .with_opponent(ConAgent::new())
        .with_judge(JudgeAgent::new())
        .with_rounds(3);

    let topic = AgentInput::text("Is Rust better than Go?");
    let result = debate.debide(&topic).await?;

    println!("Winner: {:?}", result.winner);
    println!("Reasoning: {}", result.reasoning);
    Ok(())
}
```

## Pub-Sub Pattern

Broadcast messages to multiple subscribers.

```rust
use mofa_sdk::coordination::PubSub;

#[tokio::main]
async fn main() -> Result<()> {
    let mut pubsub = PubSub::new();

    // Subscribe agents to topics
    pubsub.subscribe("news", NewsProcessor::new());
    pubsub.subscribe("news", SentimentAnalyzer::new());
    pubsub.subscribe("alerts", AlertHandler::new());

    // Broadcast
    pubsub.publish("news", AgentInput::text("Breaking news...")).await?;

    Ok(())
}
```

## Custom Coordination

Implement your own coordination pattern.

```rust
use mofa_sdk::coordination::{CoordinationPattern, CoordinationContext};

struct MyCustomPattern {
    agents: Vec<Box<dyn MoFAAgent>>,
}

#[async_trait]
impl CoordinationPattern for MyCustomPattern {
    async fn execute(&self, input: AgentInput) -> AgentResult<AgentOutput> {
        // Your custom coordination logic
        // Example: Route based on input type
        let first_output = self.agents[0].execute(input.clone(), &ctx).await?;

        // Transform and route to second agent
        let transformed = transform(first_output);
        self.agents[1].execute(transformed, &ctx).await
    }
}
```

## Running Examples

```bash
cargo run -p multi_agent_coordination
```

## Available Examples

| Example | Pattern |
|---------|---------|
| `multi_agent_coordination` | All patterns |
| `adaptive_collaboration` | Adaptive routing |
| `workflow_orchestration` | StateGraph workflows |

## See Also

- [Workflows](../concepts/workflows.md) — Workflow concepts
- [Secretary Agent](../guides/secretary-agent.md) — Human-in-the-loop
