# 运行时 API 参考

运行时层 (`mofa-runtime`) 管理智能体生命周期和执行。

## 核心组件

- [AgentRunner](runner.md) — 带生命周期管理的智能体执行
- [AgentBuilder](builder.md) — 逐步构建智能体
- [SimpleRuntime](runtime.md) — 多智能体运行时

## 概述

```rust
use mofa_sdk::runtime::AgentRunner;
use mofa_sdk::kernel::{AgentInput, AgentContext};

// 用智能体创建运行器
let mut runner = AgentRunner::new(my_agent).await?;

// 执行
let output = runner.execute(AgentInput::text("你好")).await?;

// 关闭
runner.shutdown().await?;
```

## 模块

### runner
带生命周期管理的智能体执行。

### builder
用于构建智能体的建造者模式。

### registry
智能体注册和发现。

### coordination
多智能体协调模式。

## 功能标志

| 标志 | 描述 |
|------|-------------|
| `dora` | Dora-rs 分布式运行时 |
| `monitoring` | 内置监控 |

## 另见

- [架构](../../concepts/architecture.md) — 运行时层
- [智能体](../../concepts/agents.md) — 智能体生命周期
