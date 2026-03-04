# LLM Client

Unified client for interacting with LLM providers.

## Overview

```rust
use mofa_sdk::llm::{LLMClient, openai_from_env};

let provider = openai_from_env()?;
let client = LLMClient::new(Arc::new(provider));

// Simple query
let response = client.ask("What is Rust?").await?;

// With system prompt
let response = client
    .ask_with_system("You are an expert.", "Explain ownership")
    .await?;

// Streaming
let mut stream = client.stream()
    .system("You are helpful.")
    .user("Tell a story")
    .start()
    .await?;

while let Some(chunk) = stream.next().await {
    print!("{}", chunk?);
}
```

## Methods

### ask
```rust
async fn ask(&self, prompt: &str) -> Result<String, LLMError>
```

Simple query without system prompt.

### ask_with_system
```rust
async fn ask_with_system(&self, system: &str, prompt: &str) -> Result<String, LLMError>
```

Query with system prompt.

### chat
```rust
fn chat(&self) -> ChatBuilder
```

Returns a builder for complex chat interactions.

### stream
```rust
fn stream(&self) -> StreamBuilder
```

Returns a builder for streaming responses.

## ChatBuilder

```rust
let response = client.chat()
    .system("You are helpful.")
    .user("Hello")
    .user("How are you?")
    .send()
    .await?;
```

## StreamBuilder

```rust
let stream = client.stream()
    .system("You are helpful.")
    .user("Tell a story")
    .temperature(0.8)
    .max_tokens(1000)
    .start()
    .await?;
```

## Configuration

```rust
let config = LLMConfig::builder()
    .temperature(0.7)
    .max_tokens(4096)
    .top_p(1.0)
    .build();

let client = LLMClient::with_config(provider, config);
```

## See Also

- [LLM Providers Guide](../../guides/llm-providers.md) — Provider setup
