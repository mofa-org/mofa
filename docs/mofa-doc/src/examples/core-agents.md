# Core Agents

Examples demonstrating basic agent patterns.

## Basic Echo Agent

The simplest agent that echoes input.

**Location:** `examples/echo_agent/`

```rust
use mofa_sdk::kernel::prelude::*;

struct EchoAgent;

#[async_trait]
impl MoFAAgent for EchoAgent {
    fn id(&self) -> &str { "echo" }
    fn name(&self) -> &str { "Echo Agent" }
    fn capabilities(&self) -> &AgentCapabilities {
        static CAPS: AgentCapabilities = AgentCapabilities::simple("echo");
        &CAPS
    }
    fn state(&self) -> AgentState { AgentState::Ready }

    async fn execute(&mut self, input: AgentInput, _ctx: &AgentContext) -> AgentResult<AgentOutput> {
        Ok(AgentOutput::text(format!("Echo: {}", input.to_text())))
    }
}
```

## LLM Chat Agent

Agent powered by an LLM.

**Location:** `examples/chat_stream/`

```rust
use mofa_sdk::llm::{LLMClient, openai_from_env};

#[tokio::main]
async fn main() -> Result<()> {
    dotenvy::dotenv().ok();

    let client = LLMClient::new(Arc::new(openai_from_env()?));

    // Streaming chat
    let mut stream = client.stream()
        .system("You are a helpful assistant.")
        .user("Tell me about Rust")
        .start()
        .await?;

    while let Some(chunk) = stream.next().await {
        print!("{}", chunk?);
    }

    Ok(())
}
```

## ReAct Agent

Reasoning + Acting agent with tools.

**Location:** `examples/react_agent/`

```rust
use mofa_sdk::react::ReActAgent;
use mofa_sdk::kernel::{Tool, ToolError};

// Define tools
struct CalculatorTool;
struct WeatherTool;

#[async_trait]
impl Tool for CalculatorTool {
    fn name(&self) -> &str { "calculator" }
    fn description(&self) -> &str { "Performs arithmetic" }
    async fn execute(&self, params: Value) -> Result<Value, ToolError> {
        // Implementation
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let agent = ReActAgent::builder()
        .with_llm(LLMClient::from_env()?)
        .with_tools(vec![
            Arc::new(CalculatorTool),
            Arc::new(WeatherTool::new()?),
        ])
        .with_max_iterations(5)
        .build();

    let output = agent.execute(
        AgentInput::text("What's the weather in Tokyo? Also calculate 25 * 4"),
        &ctx
    ).await?;

    println!("{}", output.as_text().unwrap());
    Ok(())
}
```

## Running Examples

```bash
# Basic chat
cargo run -p chat_stream

# ReAct agent
cargo run -p react_agent

# Secretary agent
cargo run -p secretary_agent
```

## Available Examples

| Example | Description |
|---------|-------------|
| `echo_agent` | Simple echo agent |
| `chat_stream` | Streaming LLM chat |
| `react_agent` | Reasoning + Acting |
| `secretary_agent` | Human-in-the-loop |
| `tool_routing` | Dynamic tool routing |
| `skills` | Skills system demo |

## See Also

- [Multi-Agent Coordination](multi-agent-coordination.md) — Multiple agents
- [Plugins](plugins.md) — Plugin examples
- [Tutorial](../tutorial/README.md) — Step-by-step guide
