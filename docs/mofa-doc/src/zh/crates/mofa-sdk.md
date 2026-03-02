# mofa-sdk

为用户提供主要 API 接口的统一 SDK。

## 目的

`mofa-sdk` 提供:
- 从所有层重新导出
- 跨语言绑定（UniFFI、PyO3）
- 便捷的建造者模式
- Secretary 智能体模式

## 模块组织

```rust
use mofa_sdk::{
    kernel,   // 核心抽象
    runtime,  // 运行时组件
    llm,      // LLM 集成
    plugins,  // 插件系统
};
```

## 用法

```rust
use mofa_sdk::kernel::prelude::*;
use mofa_sdk::llm::{LLMClient, openai_from_env};
use mofa_sdk::runtime::AgentRunner;

let client = LLMClient::new(Arc::new(openai_from_env()?));
let agent = MyAgent::new(client);
let mut runner = AgentRunner::new(agent).await?;
```

## 功能标志

| 标志 | 描述 |
|------|-------------|
| `openai` | OpenAI 提供商 |
| `anthropic` | Anthropic 提供商 |
| `uniffi` | 跨语言绑定 |
| `python` | 原生 Python 绑定 |

## 另见

- [快速开始](../getting-started/installation.md) — 快速开始
- [API 参考](../api-reference/kernel/README.md) — API 文档
