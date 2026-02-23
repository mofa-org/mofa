# 内核 API 参考

内核层 (`mofa-kernel`) 提供核心抽象和类型。

## 模块

### agent
核心智能体 trait 和类型。

- [`MoFAAgent`](agent.md) — 核心智能体 trait
- [`AgentContext`](context.md) — 执行上下文
- [`AgentInput` / `AgentOutput`](types.md) — 输入/输出类型

### components
智能体组件，如工具和记忆。

- [`Tool`](./components/tool.md) — 工具 trait
- [`Memory`](./components/memory.md) — 记忆 trait
- [`Reasoner`](./components/reasoner.md) — 推理接口

### plugin
插件系统接口。

- [`AgentPlugin`](./plugin.md) — 插件 trait

## 核心类型

```rust
// 智能体状态
pub enum AgentState {
    Created,
    Ready,
    Executing,
    Paused,
    Error,
    Shutdown,
}

// 能力
pub struct AgentCapabilities {
    pub tags: Vec<String>,
    pub input_type: InputType,
    pub output_type: OutputType,
    pub max_concurrency: usize,
}

// 错误处理
pub type AgentResult<T> = Result<T, AgentError>;

pub enum AgentError {
    InitializationFailed(String),
    ExecutionFailed(String),
    InvalidInput(String),
    ToolNotFound(String),
    Timeout,
    // ...
}
```

## 功能标志

内核没有可选功能——它始终提供最小化的核心。

## 另见

- [架构](../../concepts/architecture.md) — 架构概览
- [微内核设计](../../concepts/microkernel.md) — 设计原则
