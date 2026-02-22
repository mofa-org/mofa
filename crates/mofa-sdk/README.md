# MoFA SDK

MoFA (Modular Framework for Agents) SDK - A standard development toolkit for building AI agents with Rust.

## Architecture

```
mofa-sdk (标准 API 层 - SDK)
    ↓
├── mofa-kernel (微内核核心)
├── mofa-runtime (运行时)
├── mofa-foundation (业务逻辑)
└── mofa-plugins (插件系统)
```

## Public Modules

- `kernel`: core abstractions from `mofa-kernel`
- `runtime`: lifecycle and execution runtime
- `agent`: foundation agent building blocks
- `llm`: LLM integration and helpers
- `plugins`: plugin system and adapters
- `workflow`: workflow engine and DSL
- `persistence`: persistence stores and plugins
- `messaging`: message bus and contracts
- `secretary`: secretary agent pattern
- `collaboration`: multi-agent collaboration protocols
- `coordination`: scheduling and coordination utilities
- `prompt`: prompt templates and composition
- `config`: global config facade (kernel/runtime/foundation)
- `skills`: progressive disclosure skills system
- `prelude`: common imports for quick start

## Installation

Add this to your `Cargo.toml`:

```toml
[dependencies]
mofa-sdk = "0.1"
```

### Optional Features

```toml
# With dora-rs runtime support (distributed dataflow)
mofa-sdk = { version = "0.1", features = ["dora"] }

# With persistence support (database backends)
mofa-sdk = { version = "0.1", features = ["persistence-sqlite"] }
mofa-sdk = { version = "0.1", features = ["persistence-postgres"] }

# With monitoring support
mofa-sdk = { version = "0.1", features = ["monitoring"] }

# For cross-language bindings (Python, Kotlin, Swift, Java, Go)
# Use mofa-ffi instead - see below
```

## Quick Start

### Basic Agent

```rust
use mofa_sdk::kernel::{
    AgentCapabilities, AgentCapabilitiesBuilder, AgentContext, AgentInput, AgentOutput,
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

    async fn initialize(&mut self, _ctx: &AgentContext) -> AgentResult<()> {
        self.state = AgentState::Ready;
        Ok(())
    }

    async fn execute(&mut self, _input: AgentInput, _ctx: &AgentContext) -> AgentResult<AgentOutput> {
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

## Cross-Language Bindings

For Python, Kotlin, Swift, Java, and Go bindings, use the **[mofa-ffi](../mofa-ffi)** crate:

```toml
[dependencies]
mofa-ffi = { version = "0.1", features = ["uniffi"] }
```

See [mofa-ffi/README.md](../mofa-ffi/README.md) for detailed usage instructions.

## Features

| Feature | Description |
|---------|-------------|
| `dora` | Enable dora-rs runtime for distributed dataflow |
| `persistence-postgres` | PostgreSQL persistence backend |
| `persistence-mysql` | MySQL persistence backend |
| `persistence-sqlite` | SQLite persistence backend |
| `monitoring` | Enable monitoring and observability |
| `kokoro` | Enable Kokoro TTS support |

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
