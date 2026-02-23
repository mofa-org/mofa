# LLM 提供商

MoFA 支持多种 LLM 提供商，提供统一的接口。本指南介绍配置和使用方法。

## 支持的提供商

| 提供商 | 环境变量 | 特性 |
|--------|----------|------|
| OpenAI | `OPENAI_API_KEY`, `OPENAI_MODEL` | 流式输出、函数调用 |
| Anthropic | `ANTHROPIC_API_KEY`, `ANTHROPIC_MODEL` | 流式输出、超长上下文 |
| Ollama | `OPENAI_BASE_URL` | 本地推理、免费 |
| OpenRouter | `OPENAI_API_KEY`, `OPENAI_BASE_URL` | 多种模型 |
| vLLM | `OPENAI_BASE_URL` | 高性能 |

## OpenAI

### 配置

```env
OPENAI_API_KEY=sk-...
OPENAI_MODEL=gpt-4o           # 可选
OPENAI_BASE_URL=...           # 可选，用于代理
```

### 使用

```rust
use mofa_sdk::llm::{LLMClient, openai_from_env};

let provider = openai_from_env()?;
let client = LLMClient::new(Arc::new(provider));

// 简单查询
let response = client.ask("什么是 Rust？").await?;

// 带系统提示
let response = client
    .ask_with_system("你是一位 Rust 专家。", "解释所有权机制")
    .await?;

// 流式输出
let mut stream = client.stream().system("你很有帮助。").user("讲个故事").start().await?;
while let Some(chunk) = stream.next().await {
    print!("{}", chunk?);
}
```

### 可用模型

| 模型 | 描述 | 上下文长度 |
|------|------|------------|
| `gpt-4o` | 最新旗舰模型（默认） | 128K |
| `gpt-4-turbo` | 高性能 | 128K |
| `gpt-3.5-turbo` | 快速、经济 | 16K |

## Anthropic

### 配置

```env
ANTHROPIC_API_KEY=sk-ant-...
ANTHROPIC_MODEL=claude-sonnet-4-5-latest  # 可选
```

### 使用

```rust
use mofa_sdk::llm::{LLMClient, anthropic_from_env};

let provider = anthropic_from_env()?;
let client = LLMClient::new(Arc::new(provider));

let response = client
    .ask_with_system("你是 Claude，一个有帮助的 AI。", "你好！")
    .await?;
```

### 可用模型

| 模型 | 描述 | 上下文长度 |
|------|------|------------|
| `claude-sonnet-4-5-latest` | 平衡型（默认） | 200K |
| `claude-opus-4-latest` | 最强大 | 200K |
| `claude-haiku-3-5-latest` | 最快 | 200K |

## Ollama（本地）

### 安装

1. 安装 Ollama：`curl -fsSL https://ollama.ai/install.sh | sh`
2. 拉取模型：`ollama pull llama3.2`
3. 运行 Ollama：`ollama serve`

### 配置

```env
OPENAI_API_KEY=ollama
OPENAI_BASE_URL=http://localhost:11434/v1
OPENAI_MODEL=llama3.2
```

### 使用

与 OpenAI 相同（使用 OpenAI 兼容 API）：

```rust
let provider = openai_from_env()?;
let client = LLMClient::new(Arc::new(provider));
```

### 推荐模型

| 模型 | 大小 | 适用场景 |
|------|------|----------|
| `llama3.2` | 3B | 通用 |
| `llama3.1:8b` | 8B | 更高质量 |
| `mistral` | 7B | 快速响应 |
| `codellama` | 7B | 代码生成 |

## OpenRouter

### 配置

```env
OPENAI_API_KEY=sk-or-...
OPENAI_BASE_URL=https://openrouter.ai/api/v1
OPENAI_MODEL=google/gemini-2.0-flash-001
```

### 使用

```rust
let provider = openai_from_env()?;  // 使用 OPENAI_BASE_URL
let client = LLMClient::new(Arc::new(provider));
```

### 热门模型

| 模型 | 提供商 | 说明 |
|------|--------|------|
| `google/gemini-2.0-flash-001` | Google | 快速、强大 |
| `meta-llama/llama-3.1-70b-instruct` | Meta | 开源 |
| `mistralai/mistral-large` | Mistral | 欧洲 AI |

## vLLM

### 安装

```bash
pip install vllm
python -m vllm.entrypoints.openai.api_server --model meta-llama/Llama-2-7b-chat-hf
```

### 配置

```env
OPENAI_API_KEY=unused
OPENAI_BASE_URL=http://localhost:8000/v1
OPENAI_MODEL=meta-llama/Llama-2-7b-chat-hf
```

## 自定义提供商

实现 `LLMProvider` trait：

```rust
use mofa_sdk::llm::{LLMProvider, LLMResponse, LLMError};
use async_trait::async_trait;

pub struct MyCustomProvider {
    api_key: String,
    endpoint: String,
}

#[async_trait]
impl LLMProvider for MyCustomProvider {
    async fn complete(&self, prompt: &str) -> Result<String, LLMError> {
        // 你的实现
    }

    async fn complete_with_system(
        &self,
        system: &str,
        prompt: &str,
    ) -> Result<String, LLMError> {
        // 你的实现
    }

    async fn stream_complete(
        &self,
        system: &str,
        prompt: &str,
    ) -> Result<impl Stream<Item = Result<String, LLMError>>, LLMError> {
        // 可选的流式实现
    }
}
```

## 最佳实践

### API 密钥安全

```rust
// 永远不要硬编码 API 密钥
// 错误：
let key = "sk-...";

// 正确：使用环境变量
dotenvy::dotenv().ok();
let key = std::env::var("OPENAI_API_KEY")?;
```

### 错误处理

```rust
use mofa_sdk::llm::LLMError;

match client.ask(prompt).await {
    Ok(response) => println!("{}", response),
    Err(LLMError::RateLimited { retry_after }) => {
        tokio::time::sleep(Duration::from_secs(retry_after)).await;
        // 重试
    }
    Err(LLMError::InvalidApiKey) => {
        eprintln!("请检查您的 API 密钥配置");
    }
    Err(e) => {
        eprintln!("错误: {}", e);
    }
}
```

### Token 管理

```rust
// 使用滑动窗口管理上下文
let agent = LLMAgentBuilder::from_env()?
    .with_sliding_window(10)  // 保留最近 10 条消息
    .build_async()
    .await;

// 或手动计算 token
let tokens = client.count_tokens(&prompt).await?;
if tokens > 4000 {
    // 截断或总结
}
```

## 相关链接

- [LLM 设置](../getting-started/llm-setup.md) — 初始配置
- [流式输出](../guides/monitoring.md) — 流式响应
- [API 参考](../api-reference/foundation/llm.md) — LLM API 文档
