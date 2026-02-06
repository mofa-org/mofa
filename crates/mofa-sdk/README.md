# MoFA SDK

MoFA (Modular Framework for Agents) SDK - A unified development toolkit for building AI agents with Rust.

## Architecture

```
mofa-sdk (统一 API 层 - SDK)
    ↓
├── mofa-kernel (微内核核心)
├── mofa-runtime (运行时)
├── mofa-foundation (业务逻辑)
└── mofa-plugins (插件系统)
```

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
use mofa_sdk::kernel::{
    AgentCapabilities, AgentCapabilitiesBuilder, CoreAgentContext, AgentInput, AgentOutput,
    AgentResult, AgentState, MoFAAgent,
};
use mofa_sdk::runtime::run_agents;
use async_trait::async_trait;

struct MyAgent {
    id: String,
    name: String,
    caps: AgentCapabilities,
    state: AgentState,
}

impl MyAgent {
    fn new(id: &str, name: &str) -> Self {
        Self {
            id: id.to_string(),
            name: name.to_string(),
            caps: AgentCapabilitiesBuilder::new().build(),
            state: AgentState::Created,
        }
    }
}

#[async_trait]
impl MoFAAgent for MyAgent {
    fn id(&self) -> &str { &self.id }
    fn name(&self) -> &str { &self.name }
    fn capabilities(&self) -> &AgentCapabilities { &self.caps }

    async fn initialize(&mut self, _ctx: &CoreAgentContext) -> AgentResult<()> {
        self.state = AgentState::Ready;
        Ok(())
    }

    async fn execute(&mut self, _input: AgentInput, _ctx: &CoreAgentContext) -> AgentResult<AgentOutput> {
        Ok(AgentOutput::text("Hello from MyAgent"))
    }

    async fn shutdown(&mut self) -> AgentResult<()> {
        self.state = AgentState::Shutdown;
        Ok(())
    }

    fn state(&self) -> AgentState { self.state.clone() }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let agent = MyAgent::new("agent-001", "MyAgent");
    let outputs = run_agents(agent, vec![AgentInput::text("Hello")]).await?;
    println!("{}", outputs[0].to_text());
    Ok(())
}
```

### Batch Execution

```rust
use mofa_sdk::kernel::AgentInput;
use mofa_sdk::runtime::run_agents;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let agent = MyAgent::new("agent-002", "BatchAgent");
    let inputs = vec![
        AgentInput::text("task-1"),
        AgentInput::text("task-2"),
    ];
    let outputs = run_agents(agent, inputs).await?;
    for output in outputs {
        println!("{}", output.to_text());
    }
    Ok(())
}
```

### LLM Agent (Recommended)

Use the built-in LLMAgent for quick LLM interactions:

```rust
use mofa_sdk::llm::LLMAgentBuilder;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Create from environment (OPENAI_API_KEY)
    let agent = LLMAgentBuilder::from_env()?
        .with_id("my-agent")
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

MoFA provides cross-language bindings via UniFFI for Python, Kotlin, Swift, Java, and Go.

### Building with UniFFI

```bash
# Build with UniFFI and OpenAI features
cargo build --release --features "uniffi,openai" -p mofa-sdk
```

### Generating Bindings

Use the provided script:

```bash
cd crates/mofa-sdk

# Generate all bindings (Python, Kotlin, Swift, Java)
./generate-bindings.sh all

# Generate specific language
./generate-bindings.sh python
./generate-bindings.sh kotlin
./generate-bindings.sh swift
./generate-bindings.sh java
```

For Go bindings (requires separate tool):

```bash
cd crates/mofa-sdk/bindings/go

# Install uniffi-bindgen-go first
cargo install uniffi-bindgen-go --git https://github.com/NordSecurity/uniffi-bindgen-go

# Generate Go bindings
./generate-go.sh
```

### Python Quick Start

```bash
cd examples/python_bindings
export OPENAI_API_KEY=your-key-here
python 01_llm_agent.py
```

```python
from mofa import LLMAgentBuilder, MoFaError
import os

# Create an agent using the builder pattern
builder = LLMAgentBuilder.create()
builder = builder.set_id("my-agent")
builder = builder.set_name("Python Agent")
builder = builder.set_system_prompt("You are a helpful assistant.")
builder = builder.set_openai_provider(
    os.environ["OPENAI_API_KEY"],
    base_url=os.environ.get("OPENAI_BASE_URL"),
    model=os.environ.get("OPENAI_MODEL", "gpt-3.5-turbo")
)

agent = builder.build()

# Simple Q&A
response = agent.ask("What is Python?")
print(response)

# Multi-turn chat
r1 = agent.chat("My name is Alice.")
r2 = agent.chat("What's my name?")  # Remembers context

# Get history
history = agent.get_history()
agent.clear_history()
```

### Java Quick Start

```bash
cd examples/java_bindings
export OPENAI_API_KEY=your-key-here
mvn compile exec:java
```

```java
import com.mofa.*;

// Create an agent using the builder pattern
LLMAgentBuilder builder = UniFFI.INSTANCE.newLlmAgentBuilder();
builder = builder.setId("my-agent");
builder = builder.setName("Java Agent");
builder = builder.setSystemPrompt("You are a helpful assistant.");
builder = builder.setOpenaiProvider(
    System.getenv("OPENAI_API_KEY"),
    System.getenv("OPENAI_BASE_URL"),
    System.getenv().getOrDefault("OPENAI_MODEL", "gpt-3.5-turbo")
);

LLMAgent agent = builder.build();

// Simple Q&A
String response = agent.ask("What is Java?");
System.out.println(response);

// Multi-turn chat
String r1 = agent.chat("My name is Bob.");
String r2 = agent.chat("What's my name?");

// Get history
List<ChatMessage> history = agent.getHistory();
agent.clearHistory();
```

### Go Quick Start

```bash
cd examples/go_bindings
../crates/mofa-sdk/bindings/go/generate-go.sh
export OPENAI_API_KEY=your-key-here
go run 01_llm_agent.go
```

```go
package main

import (
    "fmt"
    "os"
    mofa "mofa-sdk/bindings/go"
)

func main() {
    // Create an agent using the builder pattern
    builder := mofa.NewLlmAgentBuilder()
    builder.SetId("my-agent")
    builder.SetName("Go Agent")
    builder.SetSystemPrompt("You are a helpful assistant.")
    builder.SetOpenaiProvider(
        os.Getenv("OPENAI_API_KEY"),
        os.Getenv("OPENAI_BASE_URL"),
        os.Getenv("OPENAI_MODEL"),
    )

    agent, _ := builder.Build()

    // Simple Q&A
    answer, _ := agent.Ask("What is Go?")
    fmt.Println(answer)

    // Multi-turn chat
    agent.Chat("My name is Charlie.")
    agent.Chat("What's my name?")

    // Get history
    history := agent.GetHistory()
    agent.ClearHistory()
}
```

### Kotlin Quick Start

```kotlin
import org.mofa.*

// Create an agent using the builder pattern
val builder = UniFFI.newLlmAgentBuilder()
builder.setId("my-agent")
builder.setName("Kotlin Agent")
builder.setSystemPrompt("You are a helpful assistant.")
builder.setOpenaiProvider(
    apiKey = System.getenv("OPENAI_API_KEY"),
    baseUrl = System.getenv("OPENAI_BASE_URL"),
    model = System.getenv("OPENAI_MODEL") ?: "gpt-3.5-turbo"
)

val agent = builder.build()

// Simple Q&A
val response = agent.ask("What is Kotlin?")
println(response)

// Multi-turn chat
val r1 = agent.chat("My name is Diana.")
val r2 = agent.chat("What's my name?")
```

### Swift Quick Start

```swift
import MoFA

// Create an agent using the builder pattern
let builder = try UniFFI.newLlmAgentBuilder()
try builder.setId("my-agent")
try builder.setName("Swift Agent")
try builder.setSystemPrompt("You are a helpful assistant.")
try builder.setOpenaiProvider(
    apiKey: ProcessInfo.processInfo.environment["OPENAI_API_KEY"]!,
    baseUrl: ProcessInfo.processInfo.environment["OPENAI_BASE_URL"],
    model: ProcessInfo.processInfo.environment["OPENAI_MODEL"]
)

let agent = try builder.build()

// Simple Q&A
let response = try agent.ask(question: "What is Swift?")
print(response)

// Multi-turn chat
let r1 = try agent.chat(message: "My name is Eve.")
let r2 = try agent.chat(message: "What's my name?")
```

### Available Functions (All Languages)

| Function | Description |
|----------|-------------|
| `get_version()` | Get SDK version string |
| `is_dora_available()` | Check if Dora runtime support is enabled |
| `new_llm_agent_builder()` | Create a new LLMAgentBuilder instance |

### LLMAgentBuilder Methods

| Method | Description |
|--------|-------------|
| `set_id(id)` | Set agent ID |
| `set_name(name)` | Set agent name |
| `set_system_prompt(prompt)` | Set system prompt |
| `set_temperature(temp)` | Set temperature (0.0-1.0) |
| `set_max_tokens(tokens)` | Set max tokens for response |
| `set_session_id(id)` | Set session ID |
| `set_user_id(id)` | Set user ID |
| `set_tenant_id(id)` | Set tenant ID |
| `set_context_window_size(size)` | Set context window size in rounds |
| `set_openai_provider(key, url, model)` | Configure OpenAI provider |
| `build()` | Build the LLMAgent instance |

### LLMAgent Methods

| Method | Description |
|--------|-------------|
| `agent_id()` | Get agent ID |
| `name()` | Get agent name |
| `ask(question)` | Simple Q&A (no context retention) |
| `chat(message)` | Multi-turn chat (with context retention) |
| `clear_history()` | Clear conversation history |
| `get_history()` | Get conversation history |

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
