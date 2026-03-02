# mofa-runtime

管理智能体生命周期和执行的运行时层。

## 目的

`mofa-runtime` 提供:
- `AgentRunner` 用于执行管理
- `AgentBuilder` 用于构建智能体
- `SimpleRuntime` 用于多智能体协调
- 消息总线和事件路由
- 插件管理

## 关键组件

| 组件 | 描述 |
|-----------|-------------|
| `AgentRunner` | 带生命周期执行智能体 |
| `AgentBuilder` | 逐步构建智能体 |
| `SimpleRuntime` | 多智能体运行时 |
| `PluginManager` | 管理插件 |

## 用法

```rust
use mofa_runtime::AgentRunner;
use mofa_kernel::{AgentInput, AgentContext};

let mut runner = AgentRunner::new(my_agent).await?;
let output = runner.execute(AgentInput::text("你好")).await?;
runner.shutdown().await?;
```

## 功能标志

| 标志 | 描述 |
|------|-------------|
| `dora` | Dora-rs 分布式运行时 |
| `monitoring` | 内置监控 |

## 另见

- [智能体](../concepts/agents.md) — 智能体概念
- [工作流](../concepts/workflows.md) — 工作流编排
