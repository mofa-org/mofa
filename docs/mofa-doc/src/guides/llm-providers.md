# LLM Providers

MoFA supports multiple LLM providers with a unified interface. This guide covers configuration and usage.

## Supported Providers

| Provider | Environment Variables | Features |
|----------|----------------------|----------|
| OpenAI | `OPENAI_API_KEY`, `OPENAI_MODEL` | Streaming, Function Calling |
| Anthropic | `ANTHROPIC_API_KEY`, `ANTHROPIC_MODEL` | Streaming, Extended Context |
| Ollama | `OPENAI_BASE_URL` | Local Inference, Free |
| OpenRouter | `OPENAI_API_KEY`, `OPENAI_BASE_URL` | Multiple Models |
| vLLM | `OPENAI_BASE_URL` | High Performance |

## OpenAI

### Configuration

```env
OPENAI_API_KEY=sk-...
OPENAI_MODEL=gpt-4o           # optional
OPENAI_BASE_URL=...           # optional, for proxies
```

### Usage

```rust
use mofa_sdk::llm::{LLMClient, openai_from_env};

let provider = openai_from_env()?;
let client = LLMClient::new(Arc::new(provider));

// Simple query
let response = client.ask("What is Rust?").await?;

// With system prompt
let response = client
    .ask_with_system("You are a Rust expert.", "Explain ownership")
    .await?;

// Streaming
let mut stream = client.stream().system("You are helpful.").user("Tell a story").start().await?;
while let Some(chunk) = stream.next().await {
    print!("{}", chunk?);
}
```

### Available Models

| Model | Description | Context Length |
|-------|-------------|----------------|
| `gpt-4o` | Latest flagship (default) | 128K |
| `gpt-4-turbo` | High performance | 128K |
| `gpt-3.5-turbo` | Fast, economical | 16K |

## Anthropic

### Configuration

```env
ANTHROPIC_API_KEY=sk-ant-...
ANTHROPIC_MODEL=claude-sonnet-4-5-latest  # optional
```

### Usage

```rust
use mofa_sdk::llm::{LLMClient, anthropic_from_env};

let provider = anthropic_from_env()?;
let client = LLMClient::new(Arc::new(provider));

let response = client
    .ask_with_system("You are Claude, a helpful AI.", "Hello!")
    .await?;
```

### Available Models

| Model | Description | Context Length |
|-------|-------------|----------------|
| `claude-sonnet-4-5-latest` | Balanced (default) | 200K |
| `claude-opus-4-latest` | Most capable | 200K |
| `claude-haiku-3-5-latest` | Fastest | 200K |

## Ollama (Local)

### Setup

1. Install Ollama: `curl -fsSL https://ollama.ai/install.sh | sh`
2. Pull a model: `ollama pull llama3.2`
3. Run Ollama: `ollama serve`

### Configuration

```env
OPENAI_API_KEY=ollama
OPENAI_BASE_URL=http://localhost:11434/v1
OPENAI_MODEL=llama3.2
```

### Usage

Same as OpenAI (uses OpenAI-compatible API):

```rust
let provider = openai_from_env()?;
let client = LLMClient::new(Arc::new(provider));
```

### Recommended Models

| Model | Size | Best For |
|-------|------|----------|
| `llama3.2` | 3B | General purpose |
| `llama3.1:8b` | 8B | Better quality |
| `mistral` | 7B | Fast responses |
| `codellama` | 7B | Code generation |

## OpenRouter

### Configuration

```env
OPENAI_API_KEY=sk-or-...
OPENAI_BASE_URL=https://openrouter.ai/api/v1
OPENAI_MODEL=google/gemini-2.0-flash-001
```

### Usage

```rust
let provider = openai_from_env()?;  // Uses OPENAI_BASE_URL
let client = LLMClient::new(Arc::new(provider));
```

### Popular Models

| Model | Provider | Notes |
|-------|----------|-------|
| `google/gemini-2.0-flash-001` | Google | Fast, capable |
| `meta-llama/llama-3.1-70b-instruct` | Meta | Open source |
| `mistralai/mistral-large` | Mistral | European AI |

## vLLM

### Setup

```bash
pip install vllm
python -m vllm.entrypoints.openai.api_server --model meta-llama/Llama-2-7b-chat-hf
```

### Configuration

```env
OPENAI_API_KEY=unused
OPENAI_BASE_URL=http://localhost:8000/v1
OPENAI_MODEL=meta-llama/Llama-2-7b-chat-hf
```

## Custom Provider

Implement the `LLMProvider` trait:

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
        // Your implementation
    }

    async fn complete_with_system(
        &self,
        system: &str,
        prompt: &str,
    ) -> Result<String, LLMError> {
        // Your implementation
    }

    async fn stream_complete(
        &self,
        system: &str,
        prompt: &str,
    ) -> Result<impl Stream<Item = Result<String, LLMError>>, LLMError> {
        // Optional streaming implementation
    }
}
```

## Fallback Chain

`FallbackChain` wraps multiple providers in priority order. When the active provider fails with a qualifying error (rate-limit, quota, network, timeout, auth), the next provider is tried automatically. It implements `LLMProvider`, so it is a transparent drop-in replacement everywhere a single provider is accepted.

### Request Flow

```mermaid
flowchart TD
    A([Incoming Request]) --> B{Provider 1\nCircuit Open?}

    B -- No --> C[Call Provider 1]
    B -- Yes --> D[Skip\nrecord circuit_skip]
    D --> K

    C --> E{Success?}
    E -- Yes --> F([Return Response])
    E -- No --> G{Trigger:\nshould_fallback?}

    G -- No --> H([Propagate Error])
    G -- Yes --> I[Record failure\nIncrement CB counter]
    I --> J{CB threshold\nreached?}
    J -- Yes --> JJ[Open Circuit\ncooldown starts]
    J -- No --> K{More Providers?}
    JJ --> K

    K -- Yes --> B2{Provider N\nCircuit Open?}
    K -- No --> L([Return last_error])

    B2 -- No --> C2[Call Provider N]
    B2 -- Yes --> D2[Skip\nrecord circuit_skip]
    D2 --> K2{More Providers?}

    C2 --> E2{Success?}
    E2 -- Yes --> F2([Return Response])
    E2 -- No --> G2{Last provider\nor trigger=Never?}

    G2 -- Yes --> H2([Propagate Error])
    G2 -- No --> I2[Record failure\nIncrement CB counter]
    I2 --> K2
    K2 -- Yes --> B2
    K2 -- No --> L2([Return last_error])
```

### Circuit Breaker State Machine

```mermaid
stateDiagram-v2
    [*] --> Closed : Provider created

    Closed --> Closed : success → reset failures = 0
    Closed --> Closed : failure < threshold → failures++
    Closed --> Open : failures >= threshold\nopen_until = now + cooldown

    Open --> Open : now < open_until → skip provider
    Open --> HalfOpen : now >= open_until → allow one attempt
    HalfOpen --> Closed : success → reset failures = 0
    HalfOpen --> Open : failure → reopen circuit
```

### Code Usage

```rust
use mofa_foundation::llm::{
    FallbackChain, FallbackTrigger, FallbackCondition, CircuitBreakerConfig,
};
use std::sync::Arc;

let chain = FallbackChain::builder()
    .with_circuit_breaker(CircuitBreakerConfig::default()) // 3 failures → 30s cooldown
    .add(openai_provider)                                  // primary
    .add_with_trigger(
        anthropic_provider,
        FallbackTrigger::on_conditions(vec![
            FallbackCondition::RateLimited,
            FallbackCondition::QuotaExceeded,
        ]),
    )
    .add_last(ollama_provider)                             // last resort
    .build();

let client = LLMClient::new(Arc::new(chain));
```

### YAML Configuration

```yaml
name: production-chain
circuit_breaker:
  failure_threshold: 3
  cooldown_secs: 30
providers:
  - provider: openai
    api_key: "sk-..."
  - provider: anthropic
    api_key: "sk-ant-..."
    trigger: any_error
  - provider: ollama
    base_url: "http://localhost:11434"
    trigger: never
```

```rust
let config: FallbackChainConfig = serde_yaml::from_str(yaml)?;
let chain = config.build(&registry).await?;
```

### Observability

```rust
let snap = chain.metrics();
println!("requests:  {}", snap.requests_total);
println!("fallbacks: {}", snap.fallbacks_total);

for provider in &snap.providers {
    println!(
        "{}: {} ok / {} fallbacks / {} cb-skips",
        provider.name,
        provider.successes,
        provider.fallback_failures,
        provider.circuit_breaker_skips,
    );
}
```

### Fallback Conditions

| Condition | Triggers on |
|-----------|-------------|
| `RateLimited` | HTTP 429 / rate-limit response |
| `QuotaExceeded` | Billing / quota error |
| `NetworkError` | TCP/TLS/DNS failure |
| `Timeout` | Request exceeded deadline |
| `AuthError` | Invalid API key |
| `ProviderUnavailable` | Provider does not support the model/feature |
| `ContextLengthExceeded` | Prompt too long for context window |
| `ModelNotFound` | Model does not exist on this provider |

The default trigger uses `RateLimited`, `QuotaExceeded`, `NetworkError`, `Timeout`, and `AuthError`.

---

## Best Practices

### API Key Security

```rust
// NEVER hardcode API keys
// BAD:
let key = "sk-...";

// GOOD: Use environment variables
dotenvy::dotenv().ok();
let key = std::env::var("OPENAI_API_KEY")?;
```

### Error Handling

```rust
use mofa_sdk::llm::LLMError;

match client.ask(prompt).await {
    Ok(response) => println!("{}", response),
    Err(LLMError::RateLimited { retry_after }) => {
        tokio::time::sleep(Duration::from_secs(retry_after)).await;
        // Retry
    }
    Err(LLMError::InvalidApiKey) => {
        eprintln!("Check your API key configuration");
    }
    Err(e) => {
        eprintln!("Error: {}", e);
    }
}
```

### Token Management

```rust
// Use sliding window to manage context
let agent = LLMAgentBuilder::from_env()?
    .with_sliding_window(10)  // Keep last 10 messages
    .build_async()
    .await;

// Or manual token counting
let tokens = client.count_tokens(&prompt).await?;
if tokens > 4000 {
    // Truncate or summarize
}
```

---

## Token Budget — Auto-Summarization & Graceful Halt

For long-running agents (multi-turn dialogue, ReAct loops, secretary workflows), conversation history grows over time and will eventually hit the provider's context limit. The token budget system handles this automatically:

- **Auto-compression** — trims or summarizes old history before each request when the context window approaches its limit
- **Budget enforcement** — hard token/cost caps per agent session or per day
- **`ContextLengthExceeded` recovery** — automatically compresses and retries once instead of crashing

### Quick Start

```rust
use mofa_foundation::llm::{LLMAgentBuilder, TokenBudgetConfig};
use mofa_kernel::budget::BudgetConfig;

let agent = LLMAgentBuilder::from_env()?
    .with_id("my-agent")
    .with_token_budget_config(TokenBudgetConfig {
        context_window_tokens: 8192,        // your provider's context limit
        auto_summarize_threshold: 0.8,      // compress when 80% full
        keep_recent_on_summarize: 4,        // always keep last 4 messages
        use_llm_summarize: true,            // LLM summary (false = sliding window)
        halt_on_budget_exceeded: true,
        budget: Some(
            BudgetConfig::default()
                .with_max_tokens_per_session(50_000)?
        ),
    })?
    .build();

// Works exactly like a normal agent — budget management is transparent
let response = agent.chat("Hello!").await?;
```

### `TokenBudgetConfig` Fields

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `context_window_tokens` | `usize` | `8192` | Provider's context window size in tokens |
| `auto_summarize_threshold` | `f64` | `0.8` | Fraction of window at which compression triggers (0.0–1.0) |
| `keep_recent_on_summarize` | `usize` | `4` | Number of recent messages always preserved during compression |
| `use_llm_summarize` | `bool` | `true` | `true` = LLM produces a summary; `false` = sliding-window trim |
| `halt_on_budget_exceeded` | `bool` | `true` | Return an error when budget is exceeded (false = log and proceed) |
| `budget` | `Option<BudgetConfig>` | `None` | Token/cost limits (see Budget Enforcement below) |

### Convenience Constructors

```rust
// Sliding window only — no LLM summary, no budget limits
let config = TokenBudgetConfig::sliding_window_only(8192);

// Default — LLM summarization at 80%, no hard budget limits
let config = TokenBudgetConfig::default();
```

### How Compression Works

```mermaid
flowchart TD
    A([Before each request]) --> B[Estimate tokens\nsystem + history + user msg]
    B --> C{estimated < threshold?}
    C -- Yes --> D([Send as-is\nfast path])
    C -- No --> E{use_llm_summarize?}

    E -- true --> F[LLM.chat summary prompt]
    F --> G{Success?}
    G -- Yes --> H([history = assistant summary + user msg])
    G -- No --> I([warn + keep original\nnever blocks request])

    E -- false --> J([ContextWindowManager.apply\ndrop oldest, keep last N messages])
```

> **Note:** Compression failure never blocks the request. If the summarization LLM call fails, the original history is kept and a warning is logged.

### `ContextLengthExceeded` Auto-Recovery

If the provider still returns `ContextLengthExceeded` (e.g. the history was too large before the threshold triggered), the agent performs one automatic recovery attempt:

```mermaid
flowchart TD
    A([ContextLengthExceeded]) --> B[force_compress\npop current user message]
    B --> C[Compress history to 50% of context window]
    C --> D[Push user message back]
    D --> E[send_existing_messages retry once]
    E --> F{Success?}
    F -- Yes --> G([Return response])
    F -- No --> H([Propagate ContextLengthExceeded])
```

Only one retry is attempted. If the retry also fails, the error propagates normally.

### Budget Enforcement

Use `BudgetConfig` to set hard limits on token usage per session or per day:

```rust
use mofa_kernel::budget::BudgetConfig;

// Session token limit only
let budget = BudgetConfig::default()
    .with_max_tokens_per_session(50_000)?;

// Session + daily cost limit
let budget = BudgetConfig::default()
    .with_max_tokens_per_session(50_000)?
    .with_max_cost_per_day(5.0)?;       // $5 USD/day
```

When `halt_on_budget_exceeded = true` (default), any call after the limit is reached returns:

```
Err(LLMError::Other("Budget exceeded: SessionTokensExceeded { used: 50001, limit: 50000 }"))
```

Set `halt_on_budget_exceeded = false` to log the violation and proceed anyway (useful for non-critical agents).

> **Note on cost tracking:** Token limits work fully. Cost limits require pricing data — record actual costs via a custom `LLMAgentEventHandler::after_chat_with_metadata` implementation.

### Streaming Support

The token budget is applied identically across all call paths:

| Method | Budget Check | Compression | Usage Recording |
|--------|-------------|-------------|-----------------|
| `chat()` / `chat_with_session()` | Yes | Yes — before send | Yes — after response |
| `chat_stream()` / `chat_stream_with_session()` | Yes | Yes — before stream | Yes — in stream completion |
| `chat_stream_with_full()` / `chat_stream_with_full_session()` | Yes | Yes — before stream | Yes — in stream completion |
| `ask()` / `ask_stream()` | No | No | No |

> `ask()` and `ask_stream()` are stateless single-shot methods with no session history — compression does not apply.

### Async vs Sync Construction

`LLMAgentBuilder::build()` (sync) and `build_async()` handle budget registration differently:

```rust
// build_async() — registers budget immediately, ready on first call
let agent = LLMAgentBuilder::from_env()?
    .with_token_budget_config(config)?
    .build_async()
    .await;

// build() — budget registration is deferred to the first chat() call
// (transparent to the caller, uses AtomicBool flag internally)
let agent = LLMAgentBuilder::from_env()?
    .with_token_budget_config(config)?
    .build();
```

Both behave identically from the caller's perspective.

### Request Flow Diagram

```mermaid
flowchart TD
    A([chat_with_session]) --> B[1 Lazy budget registration\nAtomicBool first call only]
    B --> C[2 check_budget]
    C --> D{Exceeded?}
    D -- Yes, halt=true --> E([Return Err Budget exceeded])
    D -- No or halt=false --> F[3 session.send message\ncompress_history_if_needed]
    F --> G{LLM call result}
    G -- Success --> H[5 record_usage]
    H --> I[6 after_chat hook]
    I --> J([Return response])
    G -- ContextLengthExceeded --> K[4 force_compress\ncompress to 50% window]
    K --> L[send_existing_messages\nretry once]
    L --> M{Retry result}
    M -- Success --> H
    M -- Failed --> N([Propagate error])
    G -- Other error --> N
```

---

## See Also

- [LLM Setup](../getting-started/llm-setup.md) — Initial configuration
- [Streaming](../guides/monitoring.md) — Streaming responses
- [API Reference](../api-reference/foundation/llm.md) — LLM API docs
