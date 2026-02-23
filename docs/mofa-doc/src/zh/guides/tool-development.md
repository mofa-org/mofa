# 工具开发

本指南介绍如何为 MoFA 智能体创建自定义工具。

## 工具接口

每个工具都需要实现 `Tool` trait：

```rust
#[async_trait]
pub trait Tool: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn parameters_schema(&self) -> Option<Value> { None }
    async fn execute(&self, params: Value) -> Result<Value, ToolError>;
}
```

## 创建简单工具

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
        "返回输入消息的原样内容。用于测试。"
    }

    fn parameters_schema(&self) -> Option<Value> {
        Some(json!({
            "type": "object",
            "properties": {
                "message": {
                    "type": "string",
                    "description": "要回显的消息"
                }
            },
            "required": ["message"]
        }))
    }

    async fn execute(&self, params: Value) -> Result<Value, ToolError> {
        let message = params["message"].as_str()
            .ok_or_else(|| ToolError::InvalidParameters("缺少 'message' 参数".into()))?;

        Ok(json!({
            "echoed": message,
            "length": message.len()
        }))
    }
}
```

## HTTP 工具

用于发起 HTTP 请求的工具：

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
        "发起 HTTP GET 请求并返回响应"
    }

    fn parameters_schema(&self) -> Option<Value> {
        Some(json!({
            "type": "object",
            "properties": {
                "url": {
                    "type": "string",
                    "format": "uri",
                    "description": "要获取的 URL"
                },
                "headers": {
                    "type": "object",
                    "description": "可选的请求头"
                }
            },
            "required": ["url"]
        }))
    }

    async fn execute(&self, params: Value) -> Result<Value, ToolError> {
        let url = params["url"].as_str()
            .ok_or_else(|| ToolError::InvalidParameters("缺少 URL".into()))?;

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

## 数据库工具

用于与数据库交互的工具：

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
        "执行只读 SQL 查询"
    }

    fn parameters_schema(&self) -> Option<Value> {
        Some(json!({
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "description": "要执行的 SELECT 查询"
                }
            },
            "required": ["query"]
        }))
    }

    async fn execute(&self, params: Value) -> Result<Value, ToolError> {
        let query = params["query"].as_str()
            .ok_or_else(|| ToolError::InvalidParameters("缺少查询语句".into()))?;

        // 安全检查：只允许 SELECT
        if !query.trim().to_uppercase().starts_with("SELECT") {
            return Err(ToolError::InvalidParameters("只允许 SELECT 查询".into()));
        }

        let rows = sqlx::query(query)
            .fetch_all(&self.pool)
            .await
            .map_err(|e| ToolError::ExecutionFailed(e.to_string()))?;

        // 将行转换为 JSON
        let results: Vec<Value> = rows.iter().map(|row| {
            // 将行转换为 JSON 值
            json!({}) // 简化示例
        }).collect();

        Ok(json!({ "results": results, "count": results.len() }))
    }
}
```

## 带状态的工具

有些工具需要维护状态：

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
        "递增和读取计数器"
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
                    "description": "要加减的值"
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

## 工具注册表

注册和管理工具：

```rust
use mofa_sdk::foundation::SimpleToolRegistry;
use std::sync::Arc;

let mut registry = SimpleToolRegistry::new();

// 注册多个工具
registry.register(Arc::new(EchoTool))?;
registry.register(Arc::new(HttpGetTool::new()))?;
registry.register(Arc::new(CounterTool::new()))?;

// 列出可用工具
for tool in registry.list_all() {
    println!("- {} : {}", tool.name(), tool.description());
}
```

## 错误处理

定义清晰的错误类型：

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

## 测试工具

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_echo_tool() {
        let tool = EchoTool;
        let params = json!({ "message": "你好" });

        let result = tool.execute(params).await.unwrap();

        assert_eq!(result["echoed"], "你好");
        assert_eq!(result["length"], 6);
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

## 最佳实践

1. **清晰的描述** — 帮助 LLM 理解何时使用你的工具
2. **Schema 验证** — 始终提供 JSON schema
3. **错误消息** — 返回有助于调试的错误
4. **超时设置** — 为外部操作设置超时
5. **幂等性** — 设计可安全重试的工具
6. **速率限制** — 遵守 API 速率限制

## 相关链接

- [工具概念](../concepts/tools.md) — 工具概述
- [智能体](../concepts/agents.md) — 在智能体中使用工具
- [示例](../examples/核心智能体.md) — 工具示例
