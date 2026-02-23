# 工具

工具使智能体能够与外部系统、API 交互并执行结构化操作。本页解释 MoFA 的工具系统。

## Tool Trait

每个工具都实现 `Tool` trait:

```rust
#[async_trait]
pub trait Tool: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn parameters_schema(&self) -> Option<Value> { None }

    async fn execute(&self, params: Value) -> Result<Value, ToolError>;
}
```

## 创建工具

### 简单工具

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
        "执行基本算术运算"
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

### 带外部 API 的工具

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
        "获取城市当前天气"
    }

    fn parameters_schema(&self) -> Option<Value> {
        Some(json!({
            "type": "object",
            "properties": {
                "city": {
                    "type": "string",
                    "description": "城市名称"
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

## 工具注册表

工具通过 `ToolRegistry` 管理:

```rust
use mofa_sdk::foundation::SimpleToolRegistry;
use std::sync::Arc;

let mut registry = SimpleToolRegistry::new();

// 注册工具
registry.register(Arc::new(CalculatorTool))?;
registry.register(Arc::new(WeatherTool::new(api_key)?))?;

// 获取工具
let tool = registry.get("calculator");

// 列出所有工具
let tools = registry.list_all();
```

## 在智能体中使用工具

### ReActAgent

ReAct（推理 + 行动）智能体自动使用工具:

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

// 智能体会自动选择和使用工具
let output = agent.execute(
    AgentInput::text("东京天气怎么样？还有计算 25 * 4"),
    &ctx
).await?;
```

### 手动工具调用

对于更多控制，您可以直接调用工具:

```rust
async fn execute(&mut self, input: AgentInput, ctx: &AgentContext) -> AgentResult<AgentOutput> {
    // 解析用户意图
    let intent = self.parse_intent(&input.to_text()).await?;

    // 选择适当的工具
    let tool = self.registry.get(&intent.tool_name)
        .ok_or(AgentError::ToolNotFound(intent.tool_name))?;

    // 执行工具
    let result = tool.execute(intent.parameters).await
        .map_err(|e| AgentError::ToolExecutionFailed(e.to_string()))?;

    // 处理结果
    let response = self.process_result(&result).await?;

    Ok(AgentOutput::text(response))
}
```

## 工具类别

工具可以分类以便组织和发现:

```rust
pub enum ToolCategory {
    DataProcessing,   // 转换、过滤、聚合
    ExternalAPI,      // 调用外部服务的 HTTP 请求
    FileSystem,       // 读取、写入、搜索文件
    Database,         // 查询、更新数据库
    Computation,      // 数学、算法
    Communication,    // 邮件、消息、通知
}
```

## 工具错误处理

```rust
pub enum ToolError {
    /// 提供的参数无效
    InvalidParameters(String),
    /// 执行失败
    ExecutionFailed(String),
    /// 执行超时
    Timeout,
    /// 资源未找到
    NotFound(String),
    /// 速率限制
    RateLimited { retry_after: u64 },
}
```

## 内置工具

MoFA 包含几个内置工具:

| 工具 | 描述 |
|------|-------------|
| `EchoTool` | 用于测试的简单回显 |
| `CalculatorTool` | 基本算术 |
| `DateTimeTool` | 日期/时间操作 |
| `JSONTool` | JSON 解析和操作 |

## 高级: 流式工具

对于长时间运行的操作，工具可以流式返回结果:

```rust
pub trait StreamingTool: Tool {
    async fn execute_stream(
        &self,
        params: Value,
    ) -> Result<impl Stream<Item = Result<Value, ToolError>>, ToolError>;
}
```

## 最佳实践

1. **清晰描述**: 编写有助于 LLM 理解何时使用的工具描述
2. **模式验证**: 始终为参数提供 JSON 模式
3. **错误消息**: 返回有助于调试的错误消息
4. **幂等性**: 尽可能设计幂等工具
5. **超时**: 为外部调用设置适当的超时

## 另见

- [工具开发指南](../guides/tool-development.md) — 创建工具的详细指南
- [智能体](agents.md) — 在智能体中使用工具
- [示例: 工具](../examples/核心智能体.md) — 工具示例
