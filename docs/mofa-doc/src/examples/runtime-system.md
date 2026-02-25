# Runtime System

Examples demonstrating MoFA's runtime system for agent lifecycle management.

## Basic Runtime API

Using the runtime API to create and manage agents.

**Location:** `examples/runtime_example/`

```rust
use mofa_sdk::kernel::{MoFAAgent, AgentContext, AgentInput, AgentOutput, AgentState};
use mofa_sdk::runtime::{AgentRunner, AgentBuilder, SimpleRuntime, run_agents};

// Define your agent
struct SimpleRuntimeAgent {
    id: String,
    name: String,
    state: AgentState,
}

#[async_trait]
impl MoFAAgent for SimpleRuntimeAgent {
    fn id(&self) -> &str { &self.id }
    fn name(&self) -> &str { &self.name }
    fn state(&self) -> AgentState { self.state.clone() }

    async fn initialize(&mut self, _ctx: &AgentContext) -> AgentResult<()> {
        self.state = AgentState::Ready;
        Ok(())
    }

    async fn execute(&mut self, input: AgentInput, _ctx: &AgentContext) -> AgentResult<AgentOutput> {
        self.state = AgentState::Executing;
        let text = input.to_text();
        self.state = AgentState::Ready;
        Ok(AgentOutput::text(format!("Processed: {}", text)))
    }

    async fn shutdown(&mut self) -> AgentResult<()> {
        self.state = AgentState::Shutdown;
        Ok(())
    }
}
```

### Batch Execution

Run multiple inputs through a single agent:

```rust
let agent = SimpleRuntimeAgent::new("agent_batch", "BatchAgent");
let inputs = vec![
    AgentInput::text("task-1"),
    AgentInput::text("task-2"),
    AgentInput::text("task-3"),
];

let outputs = run_agents(agent, inputs).await?;
for output in outputs {
    println!("Output: {}", output.to_text());
}
```

### Agent Builder Pattern

Build agents with configuration:

```rust
let mut runtime = AgentBuilder::new("agent1", "AgentOne")
    .with_capability("echo")
    .with_capability("event_handler")
    .with_agent(agent)
    .await?;

runtime.start().await?;
runtime.handle_event(AgentEvent::Custom("test".to_string(), vec![])).await?;
runtime.stop().await?;
```

## Multi-Agent Runtime

Manage multiple agents with message passing.

```rust
let runtime = SimpleRuntime::new();

// Register multiple agents
let metadata1 = AgentBuilder::new("master", "MasterAgent")
    .with_capability("master")
    .build_metadata();

let metadata2 = AgentBuilder::new("worker", "WorkerAgent")
    .with_capability("worker")
    .build_metadata();

let mut rx1 = runtime.register_agent(metadata1, config1, "master").await?;
let mut rx2 = runtime.register_agent(metadata2, config2, "worker").await?;

// Subscribe to topics
runtime.subscribe_topic("master", "commands").await?;
runtime.subscribe_topic("worker", "commands").await?;

// Send messages
let bus = runtime.message_bus();
bus.publish("commands", AgentEvent::Custom("start".to_string(), vec![])).await?;
bus.send_to("worker", AgentEvent::Custom("task".to_string(), b"data".to_vec())).await?;
```

## Message Bus Backpressure

Handling backpressure in the message bus.

**Location:** `examples/runtime_message_bus_backpressure/`

```rust
let runtime = SimpleRuntime::new();

// Register an agent with small channel capacity
let mut rx = runtime.register_agent(metadata, config, "worker").await?;

// Fill the channel to create backpressure
runtime.send_to_agent("slow-agent", AgentEvent::Custom("warmup".to_string(), vec![])).await?;

// Spawn a task that will block on full channel
let send_task = tokio::spawn({
    let bus = bus.clone();
    async move {
        bus.send_to("slow-agent", AgentEvent::Custom("blocked".to_string(), vec![])).await
    }
});

// Other operations remain responsive
timeout(Duration::from_millis(300), runtime.register_agent(other_meta, other_cfg, "observer")).await??;

// Drain to unblock
let _ = rx.recv().await;
send_task.await??;
```

### Key Points

- `send_to` blocks when receiver's channel is full
- `publish` blocks when any subscriber's channel is full
- Other runtime operations remain responsive during backpressure
- Use timeouts to detect slow consumers

## Running Examples

```bash
# Basic runtime example
cargo run -p runtime_example

# Backpressure demonstration
cargo run -p runtime_message_bus_backpressure
```

## Available Examples

| Example | Description |
|---------|-------------|
| `runtime_example` | Basic runtime API usage |
| `runtime_message_bus_backpressure` | Message bus backpressure handling |

## See Also

- [Architecture Overview](../concepts/architecture.md) — Runtime architecture
- [API Reference: Runtime](../api-reference/runtime/README.md) — Runtime API
