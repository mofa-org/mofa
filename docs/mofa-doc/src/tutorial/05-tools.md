# Chapter 5: Tools and Function Calling

> **Learning objectives:** Understand the `Tool` trait, create custom tools, register them with a `ToolRegistry`, and build a ReAct agent that reasons about when to use tools.

## Why Tools?

LLMs can generate text, but they can't perform actions — they can't calculate, search the web, or read files. **Tools** bridge this gap by giving the LLM functions it can call during a conversation.

The flow looks like this:

```
User: "What's 347 * 891?"
  ↓
LLM thinks: "I should use the calculator tool"
  ↓
LLM calls: calculator(expression="347 * 891")
  ↓
Tool returns: "309177"
  ↓
LLM responds: "347 × 891 = 309,177"
```

## The Tool Trait

Every tool in MoFA implements the `Tool` trait from `mofa-kernel`:

```rust
// crates/mofa-kernel/src/agent/components/tool.rs

#[async_trait]
pub trait Tool: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn parameters_schema(&self) -> serde_json::Value;  // JSON Schema
    async fn execute(&self, input: ToolInput, ctx: &AgentContext) -> ToolResult;

    // Optional methods with defaults:
    fn metadata(&self) -> ToolMetadata { ToolMetadata::default() }
    fn validate_input(&self, input: &ToolInput) -> AgentResult<()> { Ok(()) }
    fn requires_confirmation(&self) -> bool { false }
    fn to_llm_tool(&self) -> LLMTool;
}
```

The key methods:

- **`name()`** — The function name the LLM will use (e.g., `"calculator"`)
- **`description()`** — Explains what the tool does (the LLM reads this to decide when to use it)
- **`parameters_schema()`** — A JSON Schema describing the expected arguments
- **`execute()`** — Actually runs the tool and returns a result

### ToolInput and ToolResult

```rust
pub struct ToolInput {
    pub arguments: serde_json::Value,  // JSON arguments from the LLM
    pub raw_input: Option<String>,     // Raw string input (optional)
}

impl ToolInput {
    pub fn get<T: DeserializeOwned>(&self, key: &str) -> Option<T>;
    pub fn get_str(&self, key: &str) -> Option<&str>;
    pub fn get_number(&self, key: &str) -> Option<f64>;
    pub fn get_bool(&self, key: &str) -> Option<bool>;
}

pub struct ToolResult {
    pub success: bool,
    pub output: serde_json::Value,
    pub error: Option<String>,
    pub metadata: HashMap<String, String>,
}

impl ToolResult {
    pub fn success(output: serde_json::Value) -> Self;
    pub fn success_text(text: impl Into<String>) -> Self;
    pub fn failure(error: impl Into<String>) -> Self;
}
```

## Build: Calculator and Weather Tools

Let's create two tools and wire them up with an LLM agent.

Create a new project:

```bash
cargo new tool_agent
cd tool_agent
```

Edit `Cargo.toml`:

```toml
[package]
name = "tool_agent"
version = "0.1.0"
edition = "2024"

[dependencies]
mofa-sdk = { path = "../../crates/mofa-sdk" }
async-trait = "0.1"
tokio = { version = "1", features = ["full"] }
serde_json = "1"
```

Write `src/main.rs`:

```rust
use async_trait::async_trait;
use mofa_sdk::kernel::{
    AgentContext, Tool, ToolInput, ToolResult, ToolMetadata, LLMTool,
};
use std::sync::Arc;
use serde_json::json;

// --- Calculator Tool ---

struct CalculatorTool;

#[async_trait]
impl Tool for CalculatorTool {
    fn name(&self) -> &str {
        "calculator"
    }

    fn description(&self) -> &str {
        "Evaluate a mathematical expression. Supports +, -, *, /, and parentheses."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "expression": {
                    "type": "string",
                    "description": "The mathematical expression to evaluate, e.g. '2 + 3 * 4'"
                }
            },
            "required": ["expression"]
        })
    }

    async fn execute(&self, input: ToolInput, _ctx: &AgentContext) -> ToolResult {
        let expr = match input.get_str("expression") {
            Some(e) => e.to_string(),
            None => return ToolResult::failure("Missing 'expression' parameter"),
        };

        // Simple evaluation (in production, use a proper math parser)
        match eval_simple_expr(&expr) {
            Ok(result) => ToolResult::success_text(format!("{}", result)),
            Err(e) => ToolResult::failure(format!("Failed to evaluate '{}': {}", expr, e)),
        }
    }

    fn to_llm_tool(&self) -> LLMTool {
        LLMTool {
            name: self.name().to_string(),
            description: self.description().to_string(),
            parameters: self.parameters_schema(),
        }
    }
}

fn eval_simple_expr(expr: &str) -> Result<f64, String> {
    // Very simplified evaluator — handles basic arithmetic
    // In a real agent, use a proper expression parser like `meval`
    let expr = expr.trim();
    // Try to parse as a simple number first
    if let Ok(n) = expr.parse::<f64>() {
        return Ok(n);
    }
    // Handle basic "a op b" patterns
    for op in ['+', '-', '*', '/'] {
        if let Some(pos) = expr.rfind(op) {
            if pos > 0 {
                let left = eval_simple_expr(&expr[..pos])?;
                let right = eval_simple_expr(&expr[pos + 1..])?;
                return match op {
                    '+' => Ok(left + right),
                    '-' => Ok(left - right),
                    '*' => Ok(left * right),
                    '/' => {
                        if right == 0.0 {
                            Err("Division by zero".to_string())
                        } else {
                            Ok(left / right)
                        }
                    }
                    _ => unreachable!(),
                };
            }
        }
    }
    Err(format!("Cannot parse expression: {}", expr))
}

// --- Weather Tool (mock) ---

struct WeatherTool;

#[async_trait]
impl Tool for WeatherTool {
    fn name(&self) -> &str {
        "get_weather"
    }

    fn description(&self) -> &str {
        "Get the current weather for a city. Returns temperature and conditions."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "city": {
                    "type": "string",
                    "description": "The city name, e.g. 'San Francisco'"
                }
            },
            "required": ["city"]
        })
    }

    async fn execute(&self, input: ToolInput, _ctx: &AgentContext) -> ToolResult {
        let city = match input.get_str("city") {
            Some(c) => c.to_string(),
            None => return ToolResult::failure("Missing 'city' parameter"),
        };

        // Mock weather data (in production, call a real weather API)
        let (temp, condition) = match city.to_lowercase().as_str() {
            "san francisco" => (18, "foggy"),
            "new york" => (25, "sunny"),
            "london" => (14, "rainy"),
            "tokyo" => (28, "humid"),
            _ => (22, "partly cloudy"),
        };

        ToolResult::success(json!({
            "city": city,
            "temperature_celsius": temp,
            "condition": condition
        }))
    }

    fn to_llm_tool(&self) -> LLMTool {
        LLMTool {
            name: self.name().to_string(),
            description: self.description().to_string(),
            parameters: self.parameters_schema(),
        }
    }
}

// --- Main: Wire tools to an LLM agent ---

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create tools
    let calculator = Arc::new(CalculatorTool) as Arc<dyn Tool>;
    let weather = Arc::new(WeatherTool) as Arc<dyn Tool>;

    println!("=== Available Tools ===");
    println!("  - {} : {}", calculator.name(), calculator.description());
    println!("  - {} : {}", weather.name(), weather.description());

    // Test the tools directly
    let ctx = AgentContext::new("test-exec");

    println!("\n=== Direct Tool Calls ===");

    let result = calculator
        .execute(ToolInput::from_json(json!({"expression": "42 + 58"})), &ctx)
        .await;
    println!("calculator('42 + 58') = {:?}", result.output);

    let result = weather
        .execute(ToolInput::from_json(json!({"city": "Tokyo"})), &ctx)
        .await;
    println!("get_weather('Tokyo') = {}", result.output);

    // Show LLM tool definitions (what gets sent to the LLM API)
    println!("\n=== LLM Tool Definitions ===");
    println!("{}", serde_json::to_string_pretty(&calculator.to_llm_tool())?);

    Ok(())
}
```

Run it:

```bash
cargo run
```

## The ReAct Pattern

MoFA supports the **ReAct** (Reasoning + Acting) pattern, where an agent iteratively:

1. **Think** — Analyze the situation and plan next steps
2. **Act** — Call a tool to gather information or perform an action
3. **Observe** — Process the tool's result
4. **Repeat** — Until the task is complete

This is implemented via MoFA's ReAct module. Here's how you use it with the `ReActTool` trait:

```rust
use mofa_sdk::react::{ReActTool, spawn_react_actor};

#[async_trait]
impl ReActTool for CalculatorTool {
    fn name(&self) -> &str { "calculator" }
    fn description(&self) -> &str { "Evaluate mathematical expressions" }
    fn parameters_schema(&self) -> Option<serde_json::Value> {
        Some(json!({
            "type": "object",
            "properties": {
                "expression": { "type": "string" }
            },
            "required": ["expression"]
        }))
    }

    async fn execute(&self, input: &str) -> Result<String, String> {
        eval_simple_expr(input)
            .map(|r| r.to_string())
            .map_err(|e| e.to_string())
    }
}
```

Then use it with an LLM agent:

```rust
let agent = LLMAgentBuilder::new()
    .with_provider(Arc::new(OpenAIProvider::from_env()))
    .build();

let tools: Vec<Arc<dyn ReActTool>> = vec![
    Arc::new(CalculatorTool),
    Arc::new(WeatherTool),
];

// The ReAct actor handles the Think → Act → Observe loop automatically
let result = spawn_react_actor(
    agent,
    tools,
    "What's the weather in Tokyo and convert the temperature from C to F?"
).await?;

println!("Final answer: {}", result);
```

> **Architecture note:** The ReAct pattern is implemented in `mofa-foundation` (`crates/mofa-foundation/src/react/`). It uses the Ractor actor framework to manage the Think/Act/Observe loop. The `spawn_react_actor` function creates an actor that runs the loop until the LLM decides it has enough information to give a final answer. See `examples/react_agent/src/main.rs` for a complete example.

## Tool Registry

For managing multiple tools, use `ToolRegistry`:

```rust
use mofa_sdk::kernel::ToolRegistry;
use mofa_sdk::agent::tools::SimpleToolRegistry;

let mut registry = SimpleToolRegistry::new();
registry.register(Arc::new(CalculatorTool))?;
registry.register(Arc::new(WeatherTool))?;

// List all tools
for desc in registry.list() {
    println!("{}: {}", desc.name, desc.description);
}

// Execute by name
let result = registry.execute(
    "calculator",
    ToolInput::from_json(json!({"expression": "100 / 4"})),
    &ctx
).await?;
```

## Built-in Tools

MoFA comes with several built-in tools in `mofa-plugins`:

```rust
use mofa_sdk::plugins::tools::create_builtin_tool_plugin;

// Creates a plugin with HTTP, filesystem, shell, calculator tools
let mut tool_plugin = create_builtin_tool_plugin("my_tools")?;
tool_plugin.init_plugin().await?;
```

These include:
- **HTTP tool**: Make web requests
- **File system tool**: Read/write files
- **Shell tool**: Execute commands
- **Calculator tool**: Evaluate expressions

## Key Takeaways

- Tools give LLMs the ability to perform actions beyond text generation
- The `Tool` trait requires: `name`, `description`, `parameters_schema`, `execute`
- `ToolInput` provides typed accessors (`get_str`, `get_number`, `get_bool`)
- `ToolResult::success()` / `ToolResult::failure()` for return values
- The ReAct pattern automates the Think → Act → Observe loop
- `SimpleToolRegistry` manages collections of tools
- Built-in tools (HTTP, filesystem, shell, calculator) are available in `mofa-plugins`

---

**Next:** [Chapter 6: Multi-Agent Coordination](06-multi-agent.md) — Orchestrate multiple agents working together.

[← Back to Table of Contents](README.md)

---

**English** | [简体中文](../zh/tutorial/05-tools.md)
