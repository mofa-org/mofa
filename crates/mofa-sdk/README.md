# MoFA API

MoFA (Model-based Framework for Agents) - A unified SDK for building AI agents with Rust.

## Installation

Add this to your `Cargo.toml`:

```toml
[dependencies]
mofa-sdk = "0.1"
```

### Optional Features

```toml
# With LLM support (OpenAI, Ollama, Azure)
mofa-sdk = { version = "0.1", features = ["openai"] }

# With dora-rs runtime support (distributed dataflow)
mofa-sdk = { version = "0.1", features = ["dora"] }

# With UniFFI bindings (Python, Kotlin, Swift, Java)
mofa-sdk = { version = "0.1", features = ["uniffi"] }

# With PyO3 Python bindings
mofa-sdk = { version = "0.1", features = ["python"] }

# Full features
mofa-sdk = { version = "0.1", features = ["openai", "uniffi", "dora"] }
```

## Quick Start

### Basic Agent

```rust
use mofa_sdk::{MoFAAgent, AgentConfig, AgentEvent, AgentInterrupt, run_agent};
use async_trait::async_trait;
use std::collections::HashMap;

struct MyAgent {
    config: AgentConfig,
}

impl MyAgent {
    fn new(id: &str, name: &str) -> Self {
        Self {
            config: AgentConfig {
                agent_id: id.to_string(),
                name: name.to_string(),
                node_config: HashMap::new(),
            },
        }
    }
}

#[async_trait]
impl MoFAAgent for MyAgent {
    fn config(&self) -> &AgentConfig {
        &self.config
    }

    async fn init(&mut self, _interrupt: &AgentInterrupt) -> anyhow::Result<()> {
        info!("Agent {} initialized", self.config.agent_id);
        Ok(())
    }

    async fn handle_event(&mut self, event: AgentEvent, _interrupt: &AgentInterrupt) -> anyhow::Result<()> {
        info!("Received event: {:?}", event);
        Ok(())
    }

    async fn destroy(&mut self) -> anyhow::Result<()> {
        info!("Agent {} destroyed", self.config.agent_id);
        Ok(())
    }

    async fn on_interrupt(&mut self) -> anyhow::Result<()> {
        info!("Agent interrupted");
        Ok(())
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let agent = MyAgent::new("agent-001", "MyAgent");
    run_agent(agent).await
}
```

### LLM Agent (Recommended)

Use the built-in LLMAgent for quick LLM interactions:

```rust
use mofa_sdk::llm::{LLMAgentBuilder, openai_from_env};
use std::sync::Arc;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Create from environment (OPENAI_API_KEY)
    let agent = LLMAgentBuilder::new("my-agent")
        .with_provider(Arc::new(openai_from_env()))
        .with_system_prompt("You are a helpful assistant.")
        .build();

    // Simple Q&A
    let response = agent.ask("What is Rust?").await?;
    info!("{}", response);

    // Multi-turn chat
    let r1 = agent.chat("My name is Alice.").await?;
    let r2 = agent.chat("What's my name?").await?;  // Remembers context

    Ok(())
}
```

### Load Agent from Configuration File

```rust
use mofa_sdk::llm::agent_from_config;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Load from agent.yml
    let agent = agent_from_config("agent.yml")?;

    let response = agent.ask("Hello!").await?;
    info!("{}", response);

    Ok(())
}
```

### With Dora Runtime

```rust
use mofa_sdk::dora::{DoraRuntime, run_dataflow};

#[tokio::main]
async fn main() -> eyre::Result<()> {
    // Quick run a dataflow
    let result = run_dataflow("dataflow.yml").await?;
    info!("Dataflow {} completed", result.uuid);
    Ok(())
}
```

## Cross-Language Bindings (UniFFI)

MoFA provides cross-language bindings via UniFFI for Python, Kotlin, Swift, and Java.

### Building with UniFFI

```bash
# Build with UniFFI and OpenAI features
cargo build --release --features "uniffi,openai" -p mofa-sdk
```

### Generating Bindings

Use the provided script:

```bash
cd crates/mofa-sdk

# Generate all bindings
./generate-bindings.sh all

# Generate specific language
./generate-bindings.sh python
./generate-bindings.sh kotlin
./generate-bindings.sh swift
./generate-bindings.sh java
```

Or manually with uniffi-bindgen:

```bash
# Install uniffi-bindgen
cargo install uniffi-bindgen-cli

# Generate Python bindings
uniffi-bindgen generate \
    --library target/release/libmofa_api.dylib \
    --language python \
    --out-dir bindings/python

# Generate Kotlin bindings
uniffi-bindgen generate \
    --library target/release/libmofa_api.dylib \
    --language kotlin \
    --out-dir bindings/kotlin
```

### Using in Python

```python
from mofa import LLMAgent, LLMConfig, LLMProviderType

# From config file
agent = LLMAgent.from_config_file("agent.yml")

# Or from config dict
config = LLMConfig(
    provider=LLMProviderType.OPEN_AI,
    model="gpt-4o",
    api_key="sk-...",  # Or use OPENAI_API_KEY env var
)
agent = LLMAgent.from_config(config, "my-agent", "My Agent")

# Use the agent
response = agent.ask("What is Python?")
print(response)

# Multi-turn chat
r1 = agent.chat("Hello!")
r2 = agent.chat("What did I just say?")  # Remembers context

# Get history
history = agent.get_history()
agent.clear_history()
```

### Using in Kotlin

```kotlin
import org.mofa.LLMAgent
import org.mofa.LLMConfig
import org.mofa.LLMProviderType

// From config file
val agent = LLMAgent.fromConfigFile("agent.yml")

// Use the agent
val response = agent.ask("What is Kotlin?")
println(response)

// Multi-turn chat
val r1 = agent.chat("Hello!")
val r2 = agent.chat("What did I just say?")
```

### Using in Swift

```swift
import MoFA

// From config file
let agent = try LLMAgent.fromConfigFile(configPath: "agent.yml")

// Use the agent
let response = try agent.ask(question: "What is Swift?")
print(response)
```

## Features

| Feature | Description |
|---------|-------------|
| `openai` | Enable LLM support (OpenAI, Ollama, Azure, Compatible) |
| `dora` | Enable dora-rs runtime for distributed dataflow |
| `uniffi` | Enable UniFFI bindings for cross-language support |
| `python` | Enable PyO3 Python native bindings |

## Configuration File Format (agent.yml)

```yaml
agent:
  id: "my-agent-001"
  name: "My LLM Agent"
  description: "A helpful assistant"

llm:
  provider: openai  # openai, ollama, azure, compatible
  model: gpt-4o
  api_key: ${OPENAI_API_KEY}  # Environment variable reference
  temperature: 0.7
  max_tokens: 4096
  system_prompt: |
    You are a helpful AI assistant.
```

## Documentation

- [API Documentation](https://docs.rs/mofa-sdk)
- [GitHub Repository](https://github.com/mofa-org/mofa)

## License

Licensed under either of Apache License, Version 2.0 or MIT license at your option.
