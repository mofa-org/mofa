# 第 5 章：工具与函数调用

> **学习目标：** 理解 `Tool` trait，创建自定义工具，使用 `ToolRegistry` 注册它们，并构建一个能够推理何时使用工具的 ReAct 智能体。

## 为什么需要工具？

LLM 可以生成文本，但无法执行操作——它们不能计算、搜索网络或读取文件。**工具**弥补了这一差距，为 LLM 提供了在对话中可以调用的函数。

流程如下：

```
用户："347 * 891 等于多少？"
  ↓
LLM 思考："我应该使用计算器工具"
  ↓
LLM 调用：calculator(expression="347 * 891")
  ↓
工具返回："309177"
  ↓
LLM 回复："347 × 891 = 309,177"
```

## Tool Trait

MoFA 中的每个工具都实现了 `mofa-kernel` 中的 `Tool` trait：

```rust
// crates/mofa-kernel/src/agent/components/tool.rs

#[async_trait]
pub trait Tool: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn parameters_schema(&self) -> serde_json::Value;  // JSON Schema
    async fn execute(&self, input: ToolInput, ctx: &AgentContext) -> ToolResult;

    // 带默认实现的可选方法：
    fn metadata(&self) -> ToolMetadata { ToolMetadata::default() }
    fn validate_input(&self, input: &ToolInput) -> AgentResult<()> { Ok(()) }
    fn requires_confirmation(&self) -> bool { false }
    fn to_llm_tool(&self) -> LLMTool;
}
```

关键方法：

- **`name()`** — LLM 将使用的函数名（如 `"calculator"`）
- **`description()`** — 解释工具的功能（LLM 读取此内容来决定何时使用它）
- **`parameters_schema()`** — 描述预期参数的 JSON Schema
- **`execute()`** — 实际运行工具并返回结果

### ToolInput 和 ToolResult

```rust
pub struct ToolInput {
    pub arguments: serde_json::Value,  // 来自 LLM 的 JSON 参数
    pub raw_input: Option<String>,     // 原始字符串输入（可选）
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

## 构建：计算器和天气工具

让我们创建两个工具并将它们与 LLM 智能体连接。

创建新项目：

```bash
cargo new tool_agent
cd tool_agent
```

编辑 `Cargo.toml`：

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

编写 `src/main.rs`：

```rust
use async_trait::async_trait;
use mofa_sdk::kernel::{
    AgentContext, Tool, ToolInput, ToolResult, ToolMetadata, LLMTool,
};
use std::sync::Arc;
use serde_json::json;

// --- 计算器工具 ---

struct CalculatorTool;

#[async_trait]
impl Tool for CalculatorTool {
    fn name(&self) -> &str {
        "calculator"
    }

    fn description(&self) -> &str {
        "计算数学表达式。支持 +、-、*、/ 和括号。"
    }

    fn parameters_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "expression": {
                    "type": "string",
                    "description": "要计算的数学表达式，例如 '2 + 3 * 4'"
                }
            },
            "required": ["expression"]
        })
    }

    async fn execute(&self, input: ToolInput, _ctx: &AgentContext) -> ToolResult {
        let expr = match input.get_str("expression") {
            Some(e) => e.to_string(),
            None => return ToolResult::failure("缺少 'expression' 参数"),
        };

        // 简单计算（生产环境中请使用专业的数学解析器）
        match eval_simple_expr(&expr) {
            Ok(result) => ToolResult::success_text(format!("{}", result)),
            Err(e) => ToolResult::failure(format!("无法计算 '{}': {}", expr, e)),
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
    let expr = expr.trim();
    if let Ok(n) = expr.parse::<f64>() {
        return Ok(n);
    }
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
                            Err("除以零".to_string())
                        } else {
                            Ok(left / right)
                        }
                    }
                    _ => unreachable!(),
                };
            }
        }
    }
    Err(format!("无法解析表达式: {}", expr))
}

// --- 天气工具（模拟） ---

struct WeatherTool;

#[async_trait]
impl Tool for WeatherTool {
    fn name(&self) -> &str {
        "get_weather"
    }

    fn description(&self) -> &str {
        "获取城市的当前天气。返回温度和天气状况。"
    }

    fn parameters_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "city": {
                    "type": "string",
                    "description": "城市名称，例如 '北京'"
                }
            },
            "required": ["city"]
        })
    }

    async fn execute(&self, input: ToolInput, _ctx: &AgentContext) -> ToolResult {
        let city = match input.get_str("city") {
            Some(c) => c.to_string(),
            None => return ToolResult::failure("缺少 'city' 参数"),
        };

        // 模拟天气数据（生产环境中调用真实天气 API）
        let (temp, condition) = match city.to_lowercase().as_str() {
            "san francisco" | "旧金山" => (18, "多雾"),
            "new york" | "纽约" => (25, "晴天"),
            "london" | "伦敦" => (14, "下雨"),
            "tokyo" | "东京" => (28, "潮湿"),
            "beijing" | "北京" => (26, "晴天"),
            _ => (22, "多云"),
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

// --- 主函数：将工具连接到 LLM 智能体 ---

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 创建工具
    let calculator = Arc::new(CalculatorTool) as Arc<dyn Tool>;
    let weather = Arc::new(WeatherTool) as Arc<dyn Tool>;

    println!("=== 可用工具 ===");
    println!("  - {} : {}", calculator.name(), calculator.description());
    println!("  - {} : {}", weather.name(), weather.description());

    // 直接测试工具
    let ctx = AgentContext::new("test-exec");

    println!("\n=== 直接工具调用 ===");

    let result = calculator
        .execute(ToolInput::from_json(json!({"expression": "42 + 58"})), &ctx)
        .await;
    println!("calculator('42 + 58') = {:?}", result.output);

    let result = weather
        .execute(ToolInput::from_json(json!({"city": "东京"})), &ctx)
        .await;
    println!("get_weather('东京') = {}", result.output);

    // 显示 LLM 工具定义（发送给 LLM API 的内容）
    println!("\n=== LLM 工具定义 ===");
    println!("{}", serde_json::to_string_pretty(&calculator.to_llm_tool())?);

    Ok(())
}
```

运行它：

```bash
cargo run
```

## ReAct 模式

MoFA 支持 **ReAct**（Reasoning + Acting）模式，智能体迭代地执行：

1. **思考（Think）** — 分析情况并规划下一步
2. **行动（Act）** — 调用工具收集信息或执行操作
3. **观察（Observe）** — 处理工具的结果
4. **重复（Repeat）** — 直到任务完成

这通过 MoFA 的 ReAct 模块实现。以下是使用 `ReActTool` trait 的方式：

```rust
use mofa_sdk::react::{ReActTool, spawn_react_actor};

#[async_trait]
impl ReActTool for CalculatorTool {
    fn name(&self) -> &str { "calculator" }
    fn description(&self) -> &str { "计算数学表达式" }
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

然后与 LLM 智能体一起使用：

```rust
let agent = LLMAgentBuilder::new()
    .with_provider(Arc::new(OpenAIProvider::from_env()))
    .build();

let tools: Vec<Arc<dyn ReActTool>> = vec![
    Arc::new(CalculatorTool),
    Arc::new(WeatherTool),
];

// ReAct actor 自动处理 思考 → 行动 → 观察 循环
let result = spawn_react_actor(
    agent,
    tools,
    "东京的天气怎么样？将温度从摄氏度转换为华氏度。"
).await?;

println!("最终答案: {}", result);
```

> **架构说明：** ReAct 模式在 `mofa-foundation`（`crates/mofa-foundation/src/react/`）中实现。它使用 Ractor actor 框架管理 Think/Act/Observe 循环。`spawn_react_actor` 函数创建一个 actor 运行循环，直到 LLM 决定有足够的信息给出最终答案。参见 `examples/react_agent/src/main.rs` 获取完整示例。

## 工具注册表

使用 `ToolRegistry` 管理多个工具：

```rust
use mofa_sdk::kernel::ToolRegistry;
use mofa_sdk::agent::tools::SimpleToolRegistry;

let mut registry = SimpleToolRegistry::new();
registry.register(Arc::new(CalculatorTool))?;
registry.register(Arc::new(WeatherTool))?;

// 列出所有工具
for desc in registry.list() {
    println!("{}: {}", desc.name, desc.description);
}

// 按名称执行
let result = registry.execute(
    "calculator",
    ToolInput::from_json(json!({"expression": "100 / 4"})),
    &ctx
).await?;
```

## 内置工具

MoFA 在 `mofa-plugins` 中提供了多个内置工具：

```rust
use mofa_sdk::plugins::tools::create_builtin_tool_plugin;

// 创建包含 HTTP、文件系统、Shell、计算器工具的插件
let mut tool_plugin = create_builtin_tool_plugin("my_tools")?;
tool_plugin.init_plugin().await?;
```

包括：
- **HTTP 工具**：发起网络请求
- **文件系统工具**：读写文件
- **Shell 工具**：执行命令
- **计算器工具**：计算表达式

## 关键要点

- 工具赋予 LLM 超越文本生成的行动能力
- `Tool` trait 需要：`name`、`description`、`parameters_schema`、`execute`
- `ToolInput` 提供类型化访问器（`get_str`、`get_number`、`get_bool`）
- `ToolResult::success()` / `ToolResult::failure()` 用于返回值
- ReAct 模式自动化 思考 → 行动 → 观察 循环
- `SimpleToolRegistry` 管理工具集合
- 内置工具（HTTP、文件系统、Shell、计算器）在 `mofa-plugins` 中可用

---

**下一章：** [第 6 章：多智能体协调](06-multi-agent.md) — 编排多个智能体协同工作。

[← 返回目录](README.md)

---

[English](../../tutorial/05-tools.md) | **简体中文**
