# 智能体注册表

用于管理和发现智能体的注册表。

## 概述

`AgentRegistry` 提供:
- 智能体注册和注销
- 按能力发现智能体
- 智能体生命周期管理

## 定义

```rust
pub trait AgentRegistry: Send + Sync {
    async fn register(&mut self, agent: Box<dyn MoFAAgent>) -> AgentResult<()>;
    async fn unregister(&mut self, id: &str) -> AgentResult<()>;
    async fn get(&self, id: &str) -> Option<&dyn MoFAAgent>;
    async fn find_by_capability(&self, tag: &str) -> Vec<&dyn MoFAAgent>;
    async fn list_all(&self) -> Vec<&dyn MoFAAgent>;
}
```

## 用法

```rust
use mofa_sdk::runtime::SimpleRegistry;

let mut registry = SimpleRegistry::new();

// 注册智能体
registry.register(Box::new(ResearcherAgent::new())).await?;
registry.register(Box::new(WriterAgent::new())).await?;
registry.register(Box::new(EditorAgent::new())).await?;

// 按能力查找
let research_agents = registry.find_by_capability("research").await;

// 按 ID 获取
let agent = registry.get("researcher-1").await;

// 列出所有
for agent in registry.list_all().await {
    println!("{}", agent.name());
}
```

## SimpleRegistry

默认的内存实现:

```rust
pub struct SimpleRegistry {
    agents: HashMap<String, Box<dyn MoFAAgent>>,
}
```

## 发现

按标签或能力查找智能体:

```rust
// 按单个标签查找
let agents = registry.find_by_capability("llm").await;

// 按多个标签查找
let agents = registry.find_by_tags(&["llm", "qa"]).await;

// 按输入类型查找
let agents = registry.find_by_input_type(InputType::Text).await;
```

## 另见

- [AgentRunner](runner.md) — 智能体执行
- [智能体](../../concepts/agents.md) — 智能体概念
