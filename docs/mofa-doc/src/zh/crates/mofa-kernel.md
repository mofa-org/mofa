# mofa-kernel

提供最小抽象和类型的微内核核心。

## 目的

`mofa-kernel` 提供:
- 核心 trait 定义（`MoFAAgent`、`Tool`、`Memory`）
- 基本类型（`AgentInput`、`AgentOutput`、`AgentState`）
- 插件接口
- 事件总线原语

**重要**: 此 crate 不包含任何实现，仅包含接口。

## 关键 Trait

| Trait | 描述 |
|-------|-------------|
| `MoFAAgent` | 核心智能体接口 |
| `Tool` | 用于函数调用的工具接口 |
| `Memory` | 记忆/存储接口 |
| `AgentPlugin` | 插件接口 |

## 用法

```rust
use mofa_kernel::agent::prelude::*;

struct MyAgent { /* ... */ }

#[async_trait]
impl MoFAAgent for MyAgent {
    // 实现
}
```

## 架构规则

- ✅ 在此定义 trait
- ✅ 在此定义核心类型
- ❌ 没有实现（测试代码除外）
- ❌ 没有业务逻辑

## 功能标志

无 - 内核始终是最小化的。

## 另见

- [架构](../concepts/architecture.md) — 架构概览
- [微内核设计](../concepts/microkernel.md) — 设计原则
