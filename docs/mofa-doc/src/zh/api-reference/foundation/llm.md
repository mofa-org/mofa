# LLM 客户端

与 LLM 提供商交互的统一客户端。

## 概述

```rust
use mofa_sdk::llm::{LLMClient, openai_from_env};

let provider = openai_from_env()?;
let client = LLMClient::new(Arc::new(provider));

// 简单查询
let response = client.ask("什么是 Rust?").await?;

// 带系统提示
let response = client
    .ask_with_system("你是一个专家。", "解释所有权")
    .await?;

// 流式
let mut stream = client.stream()
    .system("你很有帮助。")
    .user("讲个故事")
    .start()
    .await?;

while let Some(chunk) = stream.next().await {
    print!("{}", chunk?);
}
```

## 方法

### ask
```rust
async fn ask(&self, prompt: &str) -> Result<String, LLMError>
```

不带系统提示的简单查询。

### ask_with_system
```rust
async fn ask_with_system(&self, system: &str, prompt: &str) -> Result<String, LLMError>
```

带系统提示的查询。

### chat
```rust
fn chat(&self) -> ChatBuilder
```

返回用于复杂聊天交互的构建器。

### stream
```rust
fn stream(&self) -> StreamBuilder
```

返回用于流式响应的构建器。

## ChatBuilder

```rust
let response = client.chat()
    .system("你很有帮助。")
    .user("你好")
    .user("你好吗?")
    .send()
    .await?;
```

## StreamBuilder

```rust
let stream = client.stream()
    .system("你很有帮助。")
    .user("讲个故事")
    .temperature(0.8)
    .max_tokens(1000)
    .start()
    .await?;
```

## 配置

```rust
let config = LLMConfig::builder()
    .temperature(0.7)
    .max_tokens(4096)
    .top_p(1.0)
    .build();

let client = LLMClient::with_config(provider, config);
```

## 另见

- [LLM 提供商指南](../../guides/llm-providers.md) — 提供商设置
