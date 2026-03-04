# Tool Development

This guide covers how to create custom tools for MoFA agents.

## Tool Interface

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

## Creating a Simple Tool

```rust
use mofa_sdk::kernel::agent::components::{Tool, ToolError};
use async_trait::async_trait;
use serde_json::{json, Value};

pub struct EchoTool;

#[async_trait]
impl Tool for EchoTool {
    fn name(&self) -> &str {
        "echo"
    }

    fn description(&self) -> &str {
        "Returns the input message unchanged. Useful for testing."
    }

    fn parameters_schema(&self) -> Option<Value> {
        Some(json!({
            "type": "object",
            "properties": {
                "message": {
                    "type": "string",
                    "description": "The message to echo back"
                }
            },
            "required": ["message"]
        }))
    }

    async fn execute(&self, params: Value) -> Result<Value, ToolError> {
        let message = params["message"].as_str()
            .ok_or_else(|| ToolError::InvalidParameters("Missing 'message' parameter".into()))?;

        Ok(json!({
            "echoed": message,
            "length": message.len()
        }))
    }
}
```

## HTTP Tool

For tools that make HTTP requests:

```rust
pub struct HttpGetTool {
    client: reqwest::Client,
    timeout: Duration,
}

impl HttpGetTool {
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::new(),
            timeout: Duration::from_secs(30),
        }
    }
}

#[async_trait]
impl Tool for HttpGetTool {
    fn name(&self) -> &str { "http_get" }

    fn description(&self) -> &str {
        "Make an HTTP GET request and return the response"
    }

    fn parameters_schema(&self) -> Option<Value> {
        Some(json!({
            "type": "object",
            "properties": {
                "url": {
                    "type": "string",
                    "format": "uri",
                    "description": "The URL to fetch"
                },
                "headers": {
                    "type": "object",
                    "description": "Optional headers"
                }
            },
            "required": ["url"]
        }))
    }

    async fn execute(&self, params: Value) -> Result<Value, ToolError> {
        let url = params["url"].as_str()
            .ok_or_else(|| ToolError::InvalidParameters("Missing URL".into()))?;

        let mut request = self.client.get(url).timeout(self.timeout);

        if let Some(headers) = params["headers"].as_object() {
            for (key, value) in headers {
                if let Some(v) = value.as_str() {
                    request = request.header(key, v);
                }
            }
        }

        let response = request.send().await
            .map_err(|e| ToolError::ExecutionFailed(e.to_string()))?;

        let status = response.status().as_u16();
        let body = response.text().await
            .map_err(|e| ToolError::ExecutionFailed(e.to_string()))?;

        Ok(json!({
            "status": status,
            "body": body
        }))
    }
}
```

## Database Tool

For tools that interact with databases:

```rust
pub struct QueryTool {
    pool: sqlx::PgPool,
}

impl QueryTool {
    pub async fn new(database_url: &str) -> Result<Self, sqlx::Error> {
        let pool = sqlx::postgres::PgPoolOptions::new()
            .max_connections(5)
            .connect(database_url)
            .await?;

        Ok(Self { pool })
    }
}

#[async_trait]
impl Tool for QueryTool {
    fn name(&self) -> &str { "database_query" }

    fn description(&self) -> &str {
        "Execute a read-only SQL query"
    }

    fn parameters_schema(&self) -> Option<Value> {
        Some(json!({
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "description": "SELECT query to execute"
                }
            },
            "required": ["query"]
        }))
    }

    async fn execute(&self, params: Value) -> Result<Value, ToolError> {
        let query = params["query"].as_str()
            .ok_or_else(|| ToolError::InvalidParameters("Missing query".into()))?;

        // Safety check: only allow SELECT
        if !query.trim().to_uppercase().starts_with("SELECT") {
            return Err(ToolError::InvalidParameters("Only SELECT queries allowed".into()));
        }

        let rows = sqlx::query(query)
            .fetch_all(&self.pool)
            .await
            .map_err(|e| ToolError::ExecutionFailed(e.to_string()))?;

        // Convert rows to JSON
        let results: Vec<Value> = rows.iter().map(|row| {
            // Convert row to JSON value
            json!({}) // Simplified
        }).collect();

        Ok(json!({ "results": results, "count": results.len() }))
    }
}
```

## Tool with State

Some tools need to maintain state:

```rust
pub struct CounterTool {
    counter: Arc<Mutex<i64>>,
}

impl CounterTool {
    pub fn new() -> Self {
        Self {
            counter: Arc::new(Mutex::new(0)),
        }
    }
}

#[async_trait]
impl Tool for CounterTool {
    fn name(&self) -> &str { "counter" }

    fn description(&self) -> &str {
        "Increment and read a counter"
    }

    fn parameters_schema(&self) -> Option<Value> {
        Some(json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["increment", "decrement", "read", "reset"]
                },
                "value": {
                    "type": "integer",
                    "description": "Value to add/subtract"
                }
            },
            "required": ["action"]
        }))
    }

    async fn execute(&self, params: Value) -> Result<Value, ToolError> {
        let action = params["action"].as_str().unwrap_or("read");
        let mut counter = self.counter.lock().await;

        match action {
            "increment" => {
                let delta = params["value"].as_i64().unwrap_or(1);
                *counter += delta;
            }
            "decrement" => {
                let delta = params["value"].as_i64().unwrap_or(1);
                *counter -= delta;
            }
            "reset" => {
                *counter = 0;
            }
            _ => {}
        }

        Ok(json!({ "value": *counter }))
    }
}
```

## Tool Registry

Register and manage tools:

```rust
use mofa_sdk::foundation::SimpleToolRegistry;
use std::sync::Arc;

let mut registry = SimpleToolRegistry::new();

// Register multiple tools
registry.register(Arc::new(EchoTool))?;
registry.register(Arc::new(HttpGetTool::new()))?;
registry.register(Arc::new(CounterTool::new()))?;

// List available tools
for tool in registry.list_all() {
    println!("- {} : {}", tool.name(), tool.description());
}
```

## Error Handling

Define clear error types:

```rust
pub enum ToolError {
    InvalidParameters(String),
    ExecutionFailed(String),
    Timeout,
    NotFound(String),
    Unauthorized,
    RateLimited { retry_after: u64 },
}
```

## Testing Tools

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_echo_tool() {
        let tool = EchoTool;
        let params = json!({ "message": "Hello" });

        let result = tool.execute(params).await.unwrap();

        assert_eq!(result["echoed"], "Hello");
        assert_eq!(result["length"], 5);
    }

    #[tokio::test]
    async fn test_missing_parameter() {
        let tool = EchoTool;
        let params = json!({});

        let result = tool.execute(params).await;

        assert!(result.is_err());
    }
}
```

## Best Practices

1. **Clear Descriptions**: Help the LLM understand when to use your tool
2. **Schema Validation**: Always provide JSON schemas
3. **Error Messages**: Return helpful errors for debugging
4. **Timeouts**: Set timeouts for external operations
5. **Idempotency**: Design tools to be safely retried
6. **Rate Limiting**: Respect API rate limits

## See Also

- [Tools Concept](../concepts/tools.md) — Tool overview
- [Agents](../concepts/agents.md) — Using tools with agents
- [Examples](../examples/core-agents.md) — Tool examples
