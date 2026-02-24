# 基础层 API 参考

基础层 (`mofa-foundation`) 提供具体实现和业务逻辑。

## 模块

### llm
LLM 客户端和提供商实现。

- `LLMClient` — 统一 LLM 客户端
- `LLMProvider` — 提供商 trait
- `OpenAIProvider` — OpenAI 实现
- `AnthropicProvider` — Anthropic 实现

### react
ReAct 智能体模式实现。

- `ReActAgent` — ReAct 智能体
- `ReActBuilder` — ReAct 智能体构建器

### secretary
用于人在回路工作流的秘书智能体模式。

- `SecretaryAgent` — 秘书智能体
- `SecretaryConfig` — 配置

### persistence
用于状态和会话管理的持久化层。

- `PersistencePlugin` — 持久化插件
- `PostgresStore` — PostgreSQL 后端
- `SqliteStore` — SQLite 后端

### coordination
多智能体协调模式。

- `Sequential` — 顺序流水线
- `Parallel` — 并行执行
- `Consensus` — 共识模式
- `Debate` — 辩论模式

## 功能标志

| 标志 | 描述 |
|------|-------------|
| `openai` | OpenAI 提供商 |
| `anthropic` | Anthropic 提供商 |
| `persistence` | 持久化层 |

## 另见

- [LLM 提供商指南](../../guides/llm-providers.md) — LLM 配置
- [持久化指南](../../guides/persistence.md) — 持久化设置
