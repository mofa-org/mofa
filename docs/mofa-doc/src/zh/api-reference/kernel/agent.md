# 智能体 Trait

`MoFAAgent` trait 是所有智能体的核心接口。

## 定义

```rust
#[async_trait]
pub trait MoFAAgent: Send + Sync {
    /// 此智能体的唯一标识符
    fn id(&self) -> &str;

    /// 人类可读的名称
    fn name(&self) -> &str;

    /// 智能体能力和元数据
    fn capabilities(&self) -> &AgentCapabilities;

    /// 当前生命周期状态
    fn state(&self) -> AgentState;

    /// 初始化智能体
    async fn initialize(&mut self, ctx: &AgentContext) -> AgentResult<()>;

    /// 执行主要智能体逻辑
    async fn execute(
        &mut self,
        input: AgentInput,
        ctx: &AgentContext,
    ) -> AgentResult<AgentOutput>;

    /// 关闭智能体
    async fn shutdown(&mut self) -> AgentResult<()>;

    // 可选的生命周期钩子
    async fn pause(&mut self) -> AgentResult<()> { Ok(()) }
    async fn resume(&mut self) -> AgentResult<()> { Ok(()) }
}
```

## 生命周期

```
Created → initialize() → Ready → execute() → Executing → Ready
                                      ↓
                               shutdown() → Shutdown
```

## 示例实现

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
        let result = format!("已处理: {}", input.to_text());
        self.state = AgentState::Ready;
        Ok(AgentOutput::text(result))
    }

    async fn shutdown(&mut self) -> AgentResult<()> {
        self.state = AgentState::Shutdown;
        Ok(())
    }
}
```

## 另见

- [AgentContext](context.md) — 执行上下文
- [AgentInput/Output](types.md) — 输入和输出类型
- [智能体概念](../../concepts/agents.md) — 智能体概述
