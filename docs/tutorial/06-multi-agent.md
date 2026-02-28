# Chapter 6: Multi-Agent Coordination

> **Learning objectives:** Understand why and when to use multiple agents, learn the 7 coordination patterns, and build chain and parallel agent pipelines.

## Why Multiple Agents?

A single agent can do a lot, but some tasks benefit from **specialization**:

- **Quality**: A "researcher" agent gathers facts, a "writer" agent crafts prose, an "editor" agent polishes — each focused on what it does best
- **Parallelism**: Multiple agents analyze different aspects of a problem simultaneously
- **Robustness**: Agents can debate or vote, reducing individual errors
- **Scalability**: Add more agents without changing existing ones

## The 7 Coordination Patterns

MoFA supports seven patterns for orchestrating multiple agents. The `CoordinationPattern` enum in `mofa-kernel` defines them:

```rust
// crates/mofa-kernel/src/agent/components/coordinator.rs

pub enum CoordinationPattern {
    Sequential,                        // Chain: A → B → C
    Parallel,                          // Fan-out: A, B, C run simultaneously
    Hierarchical { supervisor_id: String }, // Supervisor delegates to workers
    Consensus { threshold: f32 },      // Agents vote, must reach threshold
    Debate { max_rounds: usize },      // Agents argue, refine answer
    MapReduce,                         // Split task, process in parallel, merge
    Voting,                            // Majority wins
    Custom(String),                    // Your own pattern
}
```

Here's when to use each:

| Pattern | Use When | Example |
|---------|----------|---------|
| **Sequential (Chain)** | Task has natural stages | Research → Write → Edit |
| **Parallel** | Subtasks are independent | Analyze code + check security + review style |
| **Hierarchical** | Need oversight/delegation | Manager assigns tasks to specialists |
| **Consensus** | Need agreement | Multi-agent fact-checking |
| **Debate** | Quality through disagreement | Pro/con analysis, peer review |
| **MapReduce** | Large input, uniform processing | Summarize 100 documents |
| **Voting** | Simple majority decision | Classification with multiple models |

## The Coordinator Trait

The `Coordinator` trait defines how agents work together:

```rust
#[async_trait]
pub trait Coordinator: Send + Sync {
    async fn dispatch(
        &self,
        task: Task,
        ctx: &AgentContext,
    ) -> AgentResult<Vec<DispatchResult>>;

    async fn aggregate(
        &self,
        results: Vec<AgentOutput>,
    ) -> AgentResult<AgentOutput>;

    fn pattern(&self) -> CoordinationPattern;
    fn name(&self) -> &str;

    async fn select_agents(
        &self,
        task: &Task,
        ctx: &AgentContext,
    ) -> AgentResult<Vec<String>>;

    fn requires_all(&self) -> bool;
}
```

- **`dispatch`**: Sends a task to the appropriate agents
- **`aggregate`**: Combines results from multiple agents into one output
- **`select_agents`**: Decides which agents should handle a given task
- **`pattern`**: Returns the coordination strategy

## Build: Chain and Parallel Pipelines

Let's build two multi-agent examples using `MoFAAgent` implementations.

Create a new project:

```bash
cargo new multi_agent_demo
cd multi_agent_demo
```

Edit `Cargo.toml`:

```toml
[package]
name = "multi_agent_demo"
version = "0.1.0"
edition = "2024"

[dependencies]
mofa-sdk = { path = "../../crates/mofa-sdk" }
async-trait = "0.1"
tokio = { version = "1", features = ["full"] }
serde_json = "1"
```

### Example 1: Sequential Chain

Three agents in a pipeline — each transforms the output of the previous one:

```rust
use async_trait::async_trait;
use mofa_sdk::kernel::{
    AgentCapabilities, AgentCapabilitiesBuilder, AgentContext, AgentInput,
    AgentOutput, AgentResult, AgentState, MoFAAgent,
};
use mofa_sdk::runtime::run_agents;

// --- Agent that analyzes text ---
struct AnalystAgent {
    id: String,
    state: AgentState,
}

impl AnalystAgent {
    fn new() -> Self {
        Self {
            id: "analyst-001".to_string(),
            state: AgentState::Created,
        }
    }
}

#[async_trait]
impl MoFAAgent for AnalystAgent {
    fn id(&self) -> &str { &self.id }
    fn name(&self) -> &str { "Analyst" }
    fn capabilities(&self) -> &AgentCapabilities {
        &AgentCapabilitiesBuilder::new().build()
    }

    async fn initialize(&mut self, _ctx: &AgentContext) -> AgentResult<()> {
        self.state = AgentState::Ready;
        Ok(())
    }

    async fn execute(&mut self, input: AgentInput, _ctx: &AgentContext) -> AgentResult<AgentOutput> {
        let text = input.to_text();
        let analysis = format!(
            "ANALYSIS: The text '{}' has {} words and {} characters.",
            text,
            text.split_whitespace().count(),
            text.len()
        );
        Ok(AgentOutput::text(analysis))
    }

    async fn shutdown(&mut self) -> AgentResult<()> {
        self.state = AgentState::Shutdown;
        Ok(())
    }

    fn state(&self) -> AgentState { self.state.clone() }
}

// --- Agent that rewrites text ---
struct WriterAgent {
    id: String,
    state: AgentState,
}

impl WriterAgent {
    fn new() -> Self {
        Self {
            id: "writer-001".to_string(),
            state: AgentState::Created,
        }
    }
}

#[async_trait]
impl MoFAAgent for WriterAgent {
    fn id(&self) -> &str { &self.id }
    fn name(&self) -> &str { "Writer" }
    fn capabilities(&self) -> &AgentCapabilities {
        &AgentCapabilitiesBuilder::new().build()
    }

    async fn initialize(&mut self, _ctx: &AgentContext) -> AgentResult<()> {
        self.state = AgentState::Ready;
        Ok(())
    }

    async fn execute(&mut self, input: AgentInput, _ctx: &AgentContext) -> AgentResult<AgentOutput> {
        let analysis = input.to_text();
        let report = format!("REPORT:\n{}\n\nConclusion: Text processed successfully.", analysis);
        Ok(AgentOutput::text(report))
    }

    async fn shutdown(&mut self) -> AgentResult<()> {
        self.state = AgentState::Shutdown;
        Ok(())
    }

    fn state(&self) -> AgentState { self.state.clone() }
}

// --- Chain execution ---
async fn run_chain(input: &str) -> Result<String, Box<dyn std::error::Error>> {
    // Stage 1: Analyst
    let analyst = AnalystAgent::new();
    let outputs = run_agents(analyst, vec![AgentInput::text(input)]).await?;
    let analysis = outputs[0].to_text();
    println!("  [Analyst] → {}", analysis);

    // Stage 2: Writer (receives analyst's output)
    let writer = WriterAgent::new();
    let outputs = run_agents(writer, vec![AgentInput::text(&analysis)]).await?;
    let report = outputs[0].to_text();
    println!("  [Writer]  → {}", report);

    Ok(report)
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Sequential Chain: Analyst → Writer ===\n");
    let result = run_chain("MoFA is a modular agent framework built in Rust").await?;
    println!("\nFinal output:\n{}", result);

    Ok(())
}
```

### Example 2: Parallel Execution

Multiple agents process the same input concurrently, then results are aggregated:

```rust
use tokio::task::JoinSet;

async fn run_parallel(input: &str) -> Result<Vec<String, Box<dyn std::error::Error>>> {
    let mut tasks = JoinSet::new();

    // Launch multiple agents in parallel
    let input_clone = input.to_string();
    tasks.spawn(async move {
        let agent = AnalystAgent::new();
        let outputs = run_agents(agent, vec![AgentInput::text(&input_clone)]).await?;
        Ok::<_, anyhow::Error>(outputs[0].to_text())
    });

    let input_clone = input.to_string();
    tasks.spawn(async move {
        let agent = WriterAgent::new();
        let outputs = run_agents(agent, vec![AgentInput::text(&input_clone)]).await?;
        Ok::<_, anyhow::Error>(outputs[0].to_text())
    });

    // Collect results as they complete
    let mut results = Vec::new();
    while let Some(result) = tasks.join_next().await {
        match result? {
            Ok(text) => results.push(text),
            Err(e) => eprintln!("Agent failed: {}", e),
        }
    }

    Ok(results)
}
```

> **Rust tip: `JoinSet`**
> `tokio::task::JoinSet` lets you spawn multiple async tasks and collect their results as they finish. Each `spawn` returns a `JoinHandle`. `join_next().await` returns the next completed task. This is how you do parallel execution in async Rust.

## Using AgentTeam (Foundation)

For more sophisticated multi-agent coordination, MoFA's foundation layer provides `AgentTeam`:

```rust
use mofa_sdk::llm::{LLMAgentBuilder, OpenAIProvider};
use mofa_foundation::llm::multi_agent::{AgentTeam, TeamPattern};

// Create specialized LLM agents
let researcher = LLMAgentBuilder::new()
    .with_provider(provider.clone())
    .with_system_prompt("You are a thorough researcher. Gather facts.")
    .build();

let writer = LLMAgentBuilder::new()
    .with_provider(provider.clone())
    .with_system_prompt("You are a skilled writer. Create engaging content.")
    .build();

// Create a team with the builder pattern
let team = AgentTeam::new("content-team")
    .with_name("Content Team")
    .add_member("researcher", Arc::new(researcher))
    .add_member("writer", Arc::new(writer))
    .with_pattern(TeamPattern::Chain)   // Sequential pipeline
    .build();

let result = team.run("Write a blog post about Rust").await?;
```

Available `TeamPattern` values:

```rust
pub enum TeamPattern {
    Chain,                          // Output of each agent feeds into the next
    Parallel,                       // All agents run simultaneously
    Debate { max_rounds: usize },   // Agents discuss and refine over rounds
    Supervised,                     // A supervisor agent evaluates results
    MapReduce,                      // Process in parallel, then reduce
    Custom,                         // User-defined pattern (defaults to chain)
}
```

> **Architecture note:** `AgentTeam` lives in `mofa-foundation` (`crates/mofa-foundation/src/llm/multi_agent.rs`). It implements the `Coordinator` trait from `mofa-kernel` internally. See `examples/multi_agent_coordination/src/main.rs` and `examples/adaptive_collaboration_agent/src/main.rs` for complete working examples.

## What Just Happened?

In the chain example:
1. The `AnalystAgent` receives raw text and produces an analysis
2. The analysis becomes the input to the `WriterAgent`
3. The writer produces a final report

In the parallel example:
1. Both agents receive the same input simultaneously
2. They process independently (using separate OS threads via `tokio::spawn`)
3. Results are collected as they complete — no ordering guarantee

The `AgentTeam` abstraction handles this plumbing for you with LLM agents, including:
- Automatic message formatting between agents
- Error handling and retries
- Result aggregation based on the chosen pattern

## Key Takeaways

- Multi-agent coordination enables specialization, parallelism, and robustness
- 7 patterns: Sequential, Parallel, Hierarchical, Consensus, Debate, MapReduce, Voting
- `Coordinator` trait defines `dispatch`, `aggregate`, and `select_agents`
- Manual chaining: run agents sequentially, passing output as next input
- Manual parallelism: use `tokio::task::JoinSet` for concurrent execution
- `AgentTeam` provides high-level coordination for LLM agents
- `TeamPattern` selects the orchestration strategy

---

**Next:** [Chapter 7: Workflows with StateGraph](07-workflows.md) — Build stateful, graph-based workflows.

[← Back to Table of Contents](README.md)

---

**English** | [简体中文](../zh-CN/tutorial/06-multi-agent.md)
