# AgentRunner

带完整生命周期管理的智能体执行。

## 概述

`AgentRunner` 包装一个智能体并提供:
- 自动生命周期管理
- 错误处理和恢复
- 指标收集
- 优雅关闭

## 定义

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

## 用法

### 基本执行

```rust
use mofa_sdk::runtime::AgentRunner;

let mut runner = AgentRunner::new(my_agent).await?;

let output = runner.execute(AgentInput::text("你好")).await?;
println!("{}", output.as_text().unwrap());

runner.shutdown().await?;
```

### 带上下文

```rust
let ctx = AgentContext::with_session("exec-001", "session-123");
ctx.set("user_id", "user-456").await;

let mut runner = AgentRunner::with_context(my_agent, ctx).await?;
```

### 带配置

```rust
use mofa_sdk::runtime::RunnerConfig;

let config = RunnerConfig {
    timeout: Duration::from_secs(60),
    max_retries: 3,
    retry_delay: Duration::from_millis(100),
};

let runner = AgentRunner::with_config(my_agent, config);
```

### 流式执行

```rust
use futures::StreamExt;

let mut stream = runner.execute_stream(AgentInput::text("讲个故事")).await?;

while let Some(chunk) = stream.next().await {
    print!("{}", chunk);
}
```

### 批量执行

```rust
let inputs = vec![
    AgentInput::text("任务 1"),
    AgentInput::text("任务 2"),
    AgentInput::text("任务 3"),
];

for input in inputs {
    let output = runner.execute(input).await?;
    println!("{}", output.as_text().unwrap());
}
```

## 指标

```rust
let metrics = runner.metrics();
println!("执行次数: {}", metrics.total_executions);
println!("平均延迟: {:?}", metrics.avg_latency);
println!("错误数: {}", metrics.error_count);
```

## 错误处理

```rust
match runner.execute(input).await {
    Ok(output) => println!("{}", output.as_text().unwrap()),
    Err(AgentError::Timeout(d)) => {
        println!("请求在 {:?} 后超时", d);
    }
    Err(AgentError::RateLimited { retry_after }) => {
        tokio::time::sleep(Duration::from_secs(retry_after)).await;
        // 重试
    }
    Err(e) => eprintln!("错误: {}", e),
}
```

## 另见

- [运行时](README.md) — 运行时概述
- [智能体](../../concepts/agents.md) — 智能体概念
