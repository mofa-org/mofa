# mofa-foundation

提供具体实现和集成的业务层。

## 目的

`mofa-foundation` 提供:
- LLM 集成（OpenAI、Anthropic）
- 智能体模式（ReAct、Secretary）
- 持久化层
- 工作流编排
- 协作协议

## 关键模块

| 模块 | 描述 |
|--------|-------------|
| `llm` | LLM 客户端和提供商 |
| `react` | ReAct 智能体模式 |
| `secretary` | Secretary 智能体模式 |
| `persistence` | 存储后端 |
| `workflow` | 工作流编排 |
| `coordination` | 多智能体协调 |

## 用法

```rust
use mofa_foundation::llm::{LLMClient, openai_from_env};

let client = LLMClient::new(Arc::new(openai_from_env()?));
let response = client.ask("你好").await?;
```

## 功能标志

| 标志 | 描述 |
|------|-------------|
| `openai` | OpenAI 提供商 |
| `anthropic` | Anthropic 提供商 |
| `persistence` | 持久化层 |

## 架构规则

- ✅ 从 kernel 导入 trait
- ✅ 提供实现
- ❌ 永远不要重新定义 kernel trait

## 另见

- [LLM 提供商](../guides/llm-providers.md) — LLM 配置
- [持久化](../guides/persistence.md) — 持久化指南
