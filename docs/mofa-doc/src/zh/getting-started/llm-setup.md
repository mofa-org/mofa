# LLM 配置

MoFA 开箱即支持多个 LLM 提供商。本指南将帮助您配置首选的提供商。

## 支持的提供商

- **OpenAI** — GPT-4o, GPT-4-turbo, GPT-3.5-turbo
- **Anthropic** — Claude Opus, Sonnet, Haiku
- **Google Gemini** — 通过 OpenRouter
- **OpenAI 兼容端点** — Ollama, vLLM, OpenRouter 等

## 配置

在项目根目录创建 `.env` 文件。MoFA 使用 `dotenvy` 自动加载环境变量。

### OpenAI

```env
OPENAI_API_KEY=sk-...
OPENAI_MODEL=gpt-4o           # 可选，默认: gpt-4o
```

### Anthropic

```env
ANTHROPIC_API_KEY=sk-ant-...
ANTHROPIC_MODEL=claude-sonnet-4-5-latest   # 可选
```

### OpenAI 兼容端点 (Ollama, vLLM, OpenRouter)

```env
OPENAI_API_KEY=ollama          # 或您的密钥
OPENAI_BASE_URL=http://localhost:11434/v1
OPENAI_MODEL=llama3.2
```

#### 本地使用 Ollama

1. [安装 Ollama](https://ollama.ai/)
2. 拉取模型: `ollama pull llama3.2`
3. 运行 Ollama: `ollama serve`
4. 配置您的 `.env`:

```env
OPENAI_API_KEY=ollama
OPENAI_BASE_URL=http://localhost:11434/v1
OPENAI_MODEL=llama3.2
```

### Google Gemini (通过 OpenRouter)

```env
OPENAI_API_KEY=<your_openrouter_key>
OPENAI_BASE_URL=https://openrouter.ai/api/v1
OPENAI_MODEL=google/gemini-2.0-flash-001
```

## 在代码中使用 LLM

### 基本用法

```rust
use mofa_sdk::llm::{LLMClient, openai_from_env};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenvy::dotenv().ok();  // 加载 .env 文件

    let provider = openai_from_env()?;
    let client = LLMClient::new(std::sync::Arc::new(provider));

    let response = client.ask("What is Rust?").await?;
    println!("{}", response);

    Ok(())
}
```

### 使用 Chat Builder

```rust
use mofa_sdk::llm::{LLMClient, openai_from_env};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenvy::dotenv().ok();

    let provider = openai_from_env()?;
    let client = LLMClient::new(std::sync::Arc::new(provider));

    let response = client
        .chat()
        .system("You are a Rust expert.")
        .user("Explain the borrow checker.")
        .send()
        .await?;

    println!("{}", response.content().unwrap_or_default());

    Ok(())
}
```

### 流式响应

```rust
use mofa_sdk::llm::{LLMClient, openai_from_env};
use futures::StreamExt;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenvy::dotenv().ok();

    let provider = openai_from_env()?;
    let client = LLMClient::new(std::sync::Arc::new(provider));

    let mut stream = client
        .stream()
        .system("You are a helpful assistant.")
        .user("Tell me a short story.")
        .start()
        .await?;

    while let Some(chunk) = stream.next().await {
        if let Some(text) = chunk? {
            print!("{}", text);
        }
    }
    println!();

    Ok(())
}
```

## 自定义提供商

您可以通过实现 `LLMProvider` trait 来创建自己的提供商:

```rust
use mofa_sdk::llm::{LLMProvider, LLMResponse};
use async_trait::async_trait;

struct MyCustomProvider {
    api_key: String,
}

#[async_trait]
impl LLMProvider for MyCustomProvider {
    async fn complete(&self, prompt: &str) -> Result<String, Box<dyn std::error::Error>> {
        // 您的实现
        todo!()
    }

    async fn complete_with_system(
        &self,
        system: &str,
        prompt: &str,
    ) -> Result<String, Box<dyn std::error::Error>> {
        // 您的实现
        todo!()
    }
}
```

## 故障排除

### 找不到 API 密钥

确保您的 `.env` 文件位于项目根目录并包含正确的密钥名称:

```bash
# 检查 .env 是否存在
ls -la .env

# 验证内容（注意不要泄露密钥）
cat .env | grep -E "^[A-Z].*_KEY"
```

### 连接错误

- **OpenAI**: 检查您的网络连接和 API 密钥有效性
- **Ollama**: 确保 Ollama 正在运行 (`ollama serve`)
- **vLLM**: 验证 base URL 是否正确且服务器可访问

### 找不到模型

- **OpenAI**: 确保模型名称正确（例如 `gpt-4o`，而不是 `gpt-4-o`）
- **Ollama**: 先拉取模型: `ollama pull <model-name>`

## 下一步

- [构建您的第一个智能体](first-agent.md)
- [详细了解 LLM 提供商](../guides/llm-providers.md)
