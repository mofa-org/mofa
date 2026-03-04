# AgentRunner

Execute agents with full lifecycle management.

## Overview

`AgentRunner` wraps an agent and provides:
- Automatic lifecycle management
- Error handling and recovery
- Metrics collection
- Graceful shutdown

## Definition

```rust
pub struct AgentRunner<T: MoFAAgent> {
    agent: T,
    context: AgentContext,
    config: RunnerConfig,
    metrics: RunnerMetrics,
}

impl<T: MoFAAgent> AgentRunner<T> {
    pub async fn new(agent: T) -> AgentResult<Self>;
    pub async fn with_context(agent: T, context: AgentContext) -> AgentResult<Self>;
    pub fn with_config(agent: T, config: RunnerConfig) -> Self;

    pub async fn execute(&mut self, input: AgentInput) -> AgentResult<AgentOutput>;
    pub async fn execute_stream(&mut self, input: AgentInput) -> AgentResult<impl Stream<Item = String>>;

    pub async fn shutdown(&mut self) -> AgentResult<()>;
    pub fn metrics(&self) -> &RunnerMetrics;
    pub fn context(&self) -> &AgentContext;
}
```

## Usage

### Basic Execution

```rust
use mofa_sdk::runtime::AgentRunner;

let mut runner = AgentRunner::new(my_agent).await?;

let output = runner.execute(AgentInput::text("Hello")).await?;
println!("{}", output.as_text().unwrap());

runner.shutdown().await?;
```

### With Context

```rust
let ctx = AgentContext::with_session("exec-001", "session-123");
ctx.set("user_id", "user-456").await;

let mut runner = AgentRunner::with_context(my_agent, ctx).await?;
```

### With Configuration

```rust
use mofa_sdk::runtime::RunnerConfig;

let config = RunnerConfig {
    timeout: Duration::from_secs(60),
    max_retries: 3,
    retry_delay: Duration::from_millis(100),
};

let runner = AgentRunner::with_config(my_agent, config);
```

### Streaming Execution

```rust
use futures::StreamExt;

let mut stream = runner.execute_stream(AgentInput::text("Tell a story")).await?;

while let Some(chunk) = stream.next().await {
    print!("{}", chunk);
}
```

### Batch Execution

```rust
let inputs = vec![
    AgentInput::text("Task 1"),
    AgentInput::text("Task 2"),
    AgentInput::text("Task 3"),
];

for input in inputs {
    let output = runner.execute(input).await?;
    println!("{}", output.as_text().unwrap());
}
```

## Metrics

```rust
let metrics = runner.metrics();
println!("Executions: {}", metrics.total_executions);
println!("Avg latency: {:?}", metrics.avg_latency);
println!("Errors: {}", metrics.error_count);
```

## Error Handling

```rust
match runner.execute(input).await {
    Ok(output) => println!("{}", output.as_text().unwrap()),
    Err(AgentError::Timeout(d)) => {
        println!("Request timed out after {:?}", d);
    }
    Err(AgentError::RateLimited { retry_after }) => {
        tokio::time::sleep(Duration::from_secs(retry_after)).await;
        // Retry
    }
    Err(e) => eprintln!("Error: {}", e),
}
```

## See Also

- [Runtime](README.md) — Runtime overview
- [Agents](../../concepts/agents.md) — Agent concepts
