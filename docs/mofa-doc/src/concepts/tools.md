# Tools

Tools enable agents to interact with external systems, APIs, and perform structured operations. This page explains MoFA's tool system.

## The Tool Trait

Every tool implements the `Tool` trait:

```rust
#[async_trait]
pub trait Tool: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn parameters_schema(&self) -> Option<Value> { None }

    async fn execute(&self, params: Value) -> Result<Value, ToolError>;
}
```

## Creating a Tool

### Simple Tool

```rust
use mofa_sdk::kernel::agent::components::{Tool, ToolError};
use async_trait::async_trait;
use serde_json::{json, Value};

struct CalculatorTool;

#[async_trait]
impl Tool for CalculatorTool {
    fn name(&self) -> &str {
        "calculator"
    }

    fn description(&self) -> &str {
        "Performs basic arithmetic operations"
    }

    fn parameters_schema(&self) -> Option<Value> {
        Some(json!({
            "type": "object",
            "properties": {
                "operation": {
                    "type": "string",
                    "enum": ["add", "subtract", "multiply", "divide"]
                },
                "a": { "type": "number" },
                "b": { "type": "number" }
            },
            "required": ["operation", "a", "b"]
        }))
    }

    async fn execute(&self, params: Value) -> Result<Value, ToolError> {
        let op = params["operation"].as_str().unwrap_or("");
        let a = params["a"].as_f64().unwrap_or(0.0);
        let b = params["b"].as_f64().unwrap_or(0.0);

        let result = match op {
            "add" => a + b,
            "subtract" => a - b,
            "multiply" => a * b,
            "divide" => {
                if b == 0.0 {
                    return Err(ToolError::ExecutionFailed("Division by zero".into()));
                }
                a / b
            }
            _ => return Err(ToolError::InvalidParameters("Unknown operation".into())),
        };

        Ok(json!({ "result": result }))
    }
}
```

### Tool with External API

```rust
struct WeatherTool {
    api_key: String,
    client: reqwest::Client,
}

#[async_trait]
impl Tool for WeatherTool {
    fn name(&self) -> &str {
        "get_weather"
    }

    fn description(&self) -> &str {
        "Get current weather for a city"
    }

    fn parameters_schema(&self) -> Option<Value> {
        Some(json!({
            "type": "object",
            "properties": {
                "city": {
                    "type": "string",
                    "description": "City name"
                }
            },
            "required": ["city"]
        }))
    }

    async fn execute(&self, params: Value) -> Result<Value, ToolError> {
        let city = params["city"].as_str().ok_or_else(|| {
            ToolError::InvalidParameters("Missing city parameter".into())
        })?;

        let url = format!(
            "https://api.weather.com/current?city={}&key={}",
            city, self.api_key
        );

        let response = self.client
            .get(&url)
            .send()
            .await
            .map_err(|e| ToolError::ExecutionFailed(e.to_string()))?;

        let weather: Value = response
            .json()
            .await
            .map_err(|e| ToolError::ExecutionFailed(e.to_string()))?;

        Ok(weather)
    }
}
```

## Tool Registry

Tools are managed through the `ToolRegistry`:

```rust
use mofa_sdk::foundation::SimpleToolRegistry;
use std::sync::Arc;

let mut registry = SimpleToolRegistry::new();

// Register tools
registry.register(Arc::new(CalculatorTool))?;
registry.register(Arc::new(WeatherTool::new(api_key)?))?;

// Retrieve a tool
let tool = registry.get("calculator");

// List all tools
let tools = registry.list_all();
```

## Using Tools with Agents

### ReActAgent

The ReAct (Reasoning + Acting) agent uses tools automatically:

```rust
use mofa_sdk::react::ReActAgent;
use mofa_sdk::llm::openai_from_env;

let llm = LLMClient::new(Arc::new(openai_from_env()?));

let agent = ReActAgent::builder()
    .with_llm(llm)
    .with_tools(vec![
        Arc::new(CalculatorTool),
        Arc::new(WeatherTool::new(api_key)?),
    ])
    .with_max_iterations(5)
    .build();

// The agent will automatically choose and use tools
let output = agent.execute(
    AgentInput::text("What's the weather in Tokyo? Also calculate 25 * 4"),
    &ctx
).await?;
```

### Manual Tool Calling

For more control, you can call tools directly:

```rust
async fn execute(&mut self, input: AgentInput, ctx: &AgentContext) -> AgentResult<AgentOutput> {
    // Parse user intent
    let intent = self.parse_intent(&input.to_text()).await?;

    // Select appropriate tool
    let tool = self.registry.get(&intent.tool_name)
        .ok_or(AgentError::ToolNotFound(intent.tool_name))?;

    // Execute tool
    let result = tool.execute(intent.parameters).await
        .map_err(|e| AgentError::ToolExecutionFailed(e.to_string()))?;

    // Process result
    let response = self.process_result(&result).await?;

    Ok(AgentOutput::text(response))
}
```

## Tool Categories

Tools can be categorized for organization and discovery:

```rust
pub enum ToolCategory {
    DataProcessing,   // Transform, filter, aggregate
    ExternalAPI,      // HTTP calls to external services
    FileSystem,       // Read, write, search files
    Database,         // Query, update databases
    Computation,      // Math, algorithms
    Communication,    // Email, messaging, notifications
}
```

## Tool Error Handling

```rust
pub enum ToolError {
    /// Invalid parameters provided
    InvalidParameters(String),
    /// Execution failed
    ExecutionFailed(String),
    /// Timeout during execution
    Timeout,
    /// Resource not found
    NotFound(String),
    /// Rate limited
    RateLimited { retry_after: u64 },
}
```

## Built-in Tools

MoFA includes several built-in tools:

| Tool | Description |
|------|-------------|
| `EchoTool` | Simple echo for testing |
| `CalculatorTool` | Basic arithmetic |
| `DateTimeTool` | Date/time operations |
| `JSONTool` | JSON parsing and manipulation |

## Advanced: Streaming Tools

For long-running operations, tools can stream results:

```rust
pub trait StreamingTool: Tool {
    async fn execute_stream(
        &self,
        params: Value,
    ) -> Result<impl Stream<Item = Result<Value, ToolError>>, ToolError>;
}
```

## Best Practices

1. **Clear Descriptions**: Write tool descriptions that help the LLM understand when to use them
2. **Schema Validation**: Always provide JSON schemas for parameters
3. **Error Messages**: Return helpful error messages for debugging
4. **Idempotency**: Design tools to be idempotent when possible
5. **Timeouts**: Set appropriate timeouts for external calls

## See Also

- [Tool Development Guide](../guides/tool-development.md) — Detailed guide for creating tools
- [Agents](agents.md) — Using tools with agents
- [Examples: Tools](../examples/core-agents.md) — Tool examples
