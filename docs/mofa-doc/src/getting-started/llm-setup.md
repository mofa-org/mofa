# LLM Setup

MoFA supports multiple LLM providers out of the box. This guide will help you configure your preferred provider.

## Supported Providers

- **OpenAI** — GPT-4o, GPT-4-turbo, GPT-3.5-turbo
- **Anthropic** — Claude Opus, Sonnet, Haiku
- **Google Gemini** — Via OpenRouter
- **OpenAI-Compatible Endpoints** — Ollama, vLLM, OpenRouter, and more

## Configuration

Create a `.env` file in your project root. MoFA uses `dotenvy` to load environment variables automatically.

### OpenAI

```env
OPENAI_API_KEY=sk-...
OPENAI_MODEL=gpt-4o           # optional, default: gpt-4o
```

### Anthropic

```env
ANTHROPIC_API_KEY=sk-ant-...
ANTHROPIC_MODEL=claude-sonnet-4-5-latest   # optional
```

### OpenAI-Compatible Endpoints (Ollama, vLLM, OpenRouter)

```env
OPENAI_API_KEY=ollama          # or your key
OPENAI_BASE_URL=http://localhost:11434/v1
OPENAI_MODEL=llama3.2
```

#### Using Ollama Locally

1. [Install Ollama](https://ollama.ai/)
2. Pull a model: `ollama pull llama3.2`
3. Run Ollama: `ollama serve`
4. Configure your `.env`:

```env
OPENAI_API_KEY=ollama
OPENAI_BASE_URL=http://localhost:11434/v1
OPENAI_MODEL=llama3.2
```

### Google Gemini (via OpenRouter)

```env
OPENAI_API_KEY=<your_openrouter_key>
OPENAI_BASE_URL=https://openrouter.ai/api/v1
OPENAI_MODEL=google/gemini-2.0-flash-001
```

## Using LLM in Your Code

### Basic Usage

```rust
use mofa_sdk::llm::{LLMClient, openai_from_env};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenvy::dotenv().ok();  // Load .env file

    let provider = openai_from_env()?;
    let client = LLMClient::new(std::sync::Arc::new(provider));

    let response = client.ask("What is Rust?").await?;
    println!("{}", response);

    Ok(())
}
```

### With Chat Builder

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

### Streaming Responses

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

## Custom Provider

You can implement your own provider by implementing the `LLMProvider` trait:

```rust
use mofa_sdk::llm::{LLMProvider, LLMResponse};
use async_trait::async_trait;

struct MyCustomProvider {
    api_key: String,
}

#[async_trait]
impl LLMProvider for MyCustomProvider {
    async fn complete(&self, prompt: &str) -> Result<String, Box<dyn std::error::Error>> {
        // Your implementation here
        todo!()
    }

    async fn complete_with_system(
        &self,
        system: &str,
        prompt: &str,
    ) -> Result<String, Box<dyn std::error::Error>> {
        // Your implementation here
        todo!()
    }
}
```

## Troubleshooting

### API Key Not Found

Make sure your `.env` file is in the project root and contains the correct key name:

```bash
# Check if .env exists
ls -la .env

# Verify contents (be careful not to expose keys)
cat .env | grep -E "^[A-Z].*_KEY"
```

### Connection Errors

- **OpenAI**: Check your internet connection and API key validity
- **Ollama**: Ensure Ollama is running (`ollama serve`)
- **vLLM**: Verify the base URL is correct and the server is accessible

### Model Not Found

- **OpenAI**: Ensure the model name is correct (e.g., `gpt-4o`, not `gpt-4-o`)
- **Ollama**: Pull the model first: `ollama pull <model-name>`

## Next Steps

- [Build your first agent](first-agent.md)
- [Learn about LLM providers in detail](../guides/llm-providers.md)
