# Chapter 4: LLM-Powered Agent

> **Learning objectives:** Connect an agent to a real LLM, use the `LLMAgentBuilder`, handle streaming responses, and manage multi-turn conversations.

## LLM Providers in MoFA

MoFA supports four LLM providers out of the box:

| Provider | Crate | Helper function | Requires |
|----------|-------|-----------------|----------|
| **OpenAI** | `async-openai` | `OpenAIProvider::from_env()` | `OPENAI_API_KEY` |
| **Anthropic** | Custom | `AnthropicProvider::from_env()` | `ANTHROPIC_API_KEY` |
| **Google Gemini** | Custom | `GeminiProvider::from_env()` | `GOOGLE_API_KEY` |
| **Ollama** | Custom | `OllamaProvider::default()` | Ollama running locally |

All providers implement the `LLMProvider` trait from `mofa-kernel`:

```rust
#[async_trait]
pub trait LLMProvider: Send + Sync {
    fn name(&self) -> &str;
    fn default_model(&self) -> &str;
    async fn chat(&self, request: ChatCompletionRequest) -> LLMResult<ChatCompletionResponse>;
}
```

> **Architecture note:** The `LLMProvider` trait is defined in `mofa-kernel` (the contract), while `OpenAIProvider`, `OllamaProvider`, etc. live in `mofa-foundation` (the implementations). This is the microkernel pattern at work — you can create your own provider by implementing this trait.

## The LLMAgentBuilder

Instead of implementing `MoFAAgent` manually (like in Chapter 3), MoFA provides `LLMAgentBuilder` — a fluent builder that creates a fully-featured LLM agent in a few lines:

```rust
use mofa_sdk::llm::{LLMAgentBuilder, OpenAIProvider};
use std::sync::Arc;

let agent = LLMAgentBuilder::new()
    .with_id("my-agent")
    .with_name("My Assistant")
    .with_provider(Arc::new(OpenAIProvider::from_env()))
    .with_system_prompt("You are a helpful AI assistant.")
    .with_temperature(0.7)
    .with_max_tokens(2048)
    .build();
```

The builder supports many options:

| Method | Purpose |
|--------|---------|
| `.with_id(id)` | Set agent ID |
| `.with_name(name)` | Set display name |
| `.with_provider(provider)` | Set LLM provider (required) |
| `.with_system_prompt(prompt)` | Set the system prompt |
| `.with_temperature(t)` | Set sampling temperature (0.0-2.0) |
| `.with_max_tokens(n)` | Set max response tokens |
| `.with_model(model)` | Override default model name |
| `.with_session_id(id)` | Set initial session ID |
| `.with_sliding_window(n)` | Limit conversation context window |
| `.from_env()` | Auto-detect provider from env vars |

> **Rust tip: `Arc<dyn Trait>`**
> `Arc::new(OpenAIProvider::from_env())` wraps the provider in an `Arc` (atomic reference-counted pointer). This is needed because the agent and its internal components need to share the same provider. `dyn LLMProvider` means "any type that implements `LLMProvider`" — this is Rust's dynamic dispatch, similar to a virtual method call in C++ or an interface reference in Java.

## Build: A Streaming Chatbot

Let's build a chatbot that streams responses and maintains conversation context.

Create a new project:

```bash
cargo new llm_chatbot
cd llm_chatbot
```

Edit `Cargo.toml`:

```toml
[package]
name = "llm_chatbot"
version = "0.1.0"
edition = "2024"

[dependencies]
mofa-sdk = { path = "../../crates/mofa-sdk" }
tokio = { version = "1", features = ["full"] }
tokio-stream = "0.1"
```

Write `src/main.rs`:

```rust
use mofa_sdk::llm::{LLMAgentBuilder, OpenAIProvider};
use std::sync::Arc;
use tokio_stream::StreamExt;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // --- Step 1: Create the provider ---
    let provider = Arc::new(OpenAIProvider::from_env());

    // --- Step 2: Build the agent ---
    let agent = LLMAgentBuilder::new()
        .with_id("chatbot-001")
        .with_name("Tutorial Chatbot")
        .with_provider(provider)
        .with_system_prompt(
            "You are a friendly AI tutor helping students learn about \
             the MoFA agent framework. Keep answers concise."
        )
        .with_temperature(0.7)
        .build();

    // --- Step 3: Simple Q&A (non-streaming) ---
    println!("=== Simple Q&A ===");
    let response = agent.ask("What is a microkernel architecture?").await?;
    println!("A: {}\n", response);

    // --- Step 4: Streaming response ---
    println!("=== Streaming ===");
    let mut stream = agent.ask_stream("Explain traits in Rust in 3 sentences.").await?;
    print!("A: ");
    while let Some(chunk) = stream.next().await {
        match chunk {
            Ok(text) => print!("{}", text),
            Err(e) => eprintln!("\nStream error: {}", e),
        }
    }
    println!("\n");

    // --- Step 5: Multi-turn conversation ---
    println!("=== Multi-turn Chat ===");
    let r1 = agent.chat("My name is Alice and I'm learning Rust.").await?;
    println!("A: {}\n", r1);

    let r2 = agent.chat("What's my name and what am I learning?").await?;
    println!("A: {}\n", r2);
    // The agent remembers context from the previous message!

    Ok(())
}
```

Run it:

```bash
cargo run
```

## Using Ollama Instead

To use a local Ollama model, just swap the provider:

```rust
use mofa_sdk::llm::{LLMAgentBuilder, OllamaProvider};

let provider = Arc::new(OllamaProvider::default());
// Ollama uses http://localhost:11434 by default

let agent = LLMAgentBuilder::new()
    .with_provider(provider)
    .with_model("llama3.2")  // specify which Ollama model to use
    .with_system_prompt("You are a helpful assistant.")
    .build();
```

Or use the `from_env()` convenience method which auto-detects the provider:

```rust
// Checks OPENAI_API_KEY, ANTHROPIC_API_KEY, GOOGLE_API_KEY,
// and falls back to Ollama if none are set
let builder = LLMAgentBuilder::from_env()?;
let agent = builder
    .with_system_prompt("You are a helpful assistant.")
    .build();
```

## What Just Happened?

Let's trace what happens when you call `agent.ask("question")`:

1. The `LLMAgent` wraps your question in a `ChatMessage` with role `"user"`
2. It prepends the system prompt as a `ChatMessage` with role `"system"`
3. It builds a `ChatCompletionRequest` with temperature, max_tokens, etc.
4. It calls `provider.chat(request)` which sends the request to the LLM API
5. The response `ChatCompletionResponse` is unwrapped and the text content is returned

For `agent.chat()` (multi-turn), the agent also:
- Stores the user message in the current `ChatSession`
- Stores the assistant's response
- Includes all previous messages in the next request (conversation context)

For `agent.ask_stream()` and `agent.chat_stream()`:
- The provider returns a `TextStream` (a stream of string chunks)
- You consume it with `StreamExt::next()` in a loop
- Each chunk contains a piece of the response as it's generated

> **Architecture note:** The `LLMAgent` struct lives in `mofa-foundation` (`crates/mofa-foundation/src/llm/agent.rs`). It implements the `MoFAAgent` trait internally, so it has the same lifecycle (initialize → execute → shutdown). The builder pattern is a convenience — under the hood, it constructs an `LLMAgentConfig` and passes it to `LLMAgent::new()`.

## Session Management

Each `LLMAgent` manages multiple chat sessions. This is useful for serving multiple users or maintaining separate conversation threads:

```rust
// Create a new session (returns session ID)
let session_id = agent.create_session().await;

// Chat within a specific session
let r1 = agent.chat_with_session(&session_id, "Hello!").await?;

// Switch the active session
agent.switch_session(&session_id).await?;

// List all sessions
let sessions = agent.list_sessions().await;

// Get or create a session with a specific ID
let sid = agent.get_or_create_session("user-123-session").await;
```

## Loading from a Config File

For production use, you can define agent configuration in YAML:

```yaml
# agent.yml
agent:
  id: "my-agent-001"
  name: "My LLM Agent"
  description: "A helpful assistant"

llm:
  provider: openai
  model: gpt-4o
  api_key: ${OPENAI_API_KEY}
  temperature: 0.7
  max_tokens: 4096
  system_prompt: |
    You are a helpful AI assistant.
```

Load it in code:

```rust
use mofa_sdk::llm::agent_from_config;

let agent = agent_from_config("agent.yml")?;
let response = agent.ask("Hello!").await?;
```

## Key Takeaways

- `LLMAgentBuilder` is the recommended way to create LLM-powered agents
- Four providers are supported: OpenAI, Anthropic, Gemini, Ollama
- `agent.ask()` for one-off questions, `agent.chat()` for multi-turn conversations
- `agent.ask_stream()` / `agent.chat_stream()` for streaming responses
- Session management enables multi-user and multi-thread conversations
- `from_env()` auto-detects the provider from environment variables
- Config files (`agent.yml`) are supported for production deployments

---

**Next:** [Chapter 5: Tools and Function Calling](05-tools.md) — Give your agent the ability to call functions.

[← Back to Table of Contents](README.md)

---

**English** | [简体中文](../zh/tutorial/04-llm-agent.md)
