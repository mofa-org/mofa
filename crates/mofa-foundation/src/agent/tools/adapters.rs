//! 工具适配器
//! Tool adapters
//!
//! 提供便捷的工具创建方式
//! Providing convenient ways to create tools

use async_trait::async_trait;
use mofa_kernel::agent::Tool;
use mofa_kernel::agent::components::tool::{ToolInput, ToolMetadata, ToolResult};
use mofa_kernel::agent::context::AgentContext;
use std::future::Future;
use std::pin::Pin;

/// 函数工具
/// Function Tool
///
/// 从函数创建工具
/// Create tools from functions
///
/// # 示例
///
/// ```rust,ignore
/// use mofa_foundation::agent::tools::FunctionTool;
///
/// async fn my_tool_fn(input: ToolInput, ctx: &AgentContext) -> ToolResult {
///     let message = input.get_str("message").unwrap_or("default");
///     ToolResult::success_text(format!("Processed: {}", message))
/// }
///
/// let tool = FunctionTool::new(
///     "my_tool",
///     "A custom tool",
///     serde_json::json!({
///         "type": "object",
///         "properties": {
///             "message": { "type": "string" }
///         }
///     }),
///     my_tool_fn,
/// );
/// ```
pub struct FunctionTool<F>
where
    F: Fn(ToolInput, &AgentContext) -> Pin<Box<dyn Future<Output = ToolResult> + Send + '_>>
        + Send
        + Sync,
{
    name: String,
    description: String,
    parameters_schema: serde_json::Value,
    handler: F,
    metadata: ToolMetadata,
}

impl<F> FunctionTool<F>
where
    F: Fn(ToolInput, &AgentContext) -> Pin<Box<dyn Future<Output = ToolResult> + Send + '_>>
        + Send
        + Sync,
{
    /// 创建新的函数工具
    /// Create a new function tool
    pub fn new(
        name: impl Into<String>,
        description: impl Into<String>,
        parameters_schema: serde_json::Value,
        handler: F,
    ) -> Self {
        Self {
            name: name.into(),
            description: description.into(),
            parameters_schema,
            handler,
            metadata: ToolMetadata::default(),
        }
    }

    /// 设置元数据
    /// Set metadata
    pub fn with_metadata(mut self, metadata: ToolMetadata) -> Self {
        self.metadata = metadata;
        self
    }
}

#[async_trait]
impl<F> Tool for FunctionTool<F>
where
    F: Fn(ToolInput, &AgentContext) -> Pin<Box<dyn Future<Output = ToolResult> + Send + '_>>
        + Send
        + Sync,
{
    fn name(&self) -> &str {
        &self.name
    }

    fn description(&self) -> &str {
        &self.description
    }

    fn parameters_schema(&self) -> serde_json::Value {
        self.parameters_schema.clone()
    }

    async fn execute(&self, input: ToolInput, ctx: &AgentContext) -> ToolResult {
        (self.handler)(input, ctx).await
    }

    fn metadata(&self) -> ToolMetadata {
        self.metadata.clone()
    }
}

/// 闭包工具
/// Closure Tool
///
/// 使用闭包创建简单工具
/// Create simple tools using closures
///
/// # 示例
///
/// ```rust,ignore
/// use mofa_foundation::agent::tools::ClosureTool;
///
/// let tool = ClosureTool::new(
///     "add",
///     "Add two numbers",
///     |input| {
///         let a = input.get_number("a").unwrap_or(0.0);
///         let b = input.get_number("b").unwrap_or(0.0);
///         ToolResult::success_text(format!("{}", a + b))
///     },
/// );
/// ```
pub struct ClosureTool<F>
where
    F: Fn(ToolInput) -> ToolResult + Send + Sync,
{
    name: String,
    description: String,
    parameters_schema: serde_json::Value,
    handler: F,
    metadata: ToolMetadata,
}

impl<F> ClosureTool<F>
where
    F: Fn(ToolInput) -> ToolResult + Send + Sync,
{
    /// 创建新的闭包工具
    /// Create a new closure tool
    pub fn new(name: impl Into<String>, description: impl Into<String>, handler: F) -> Self {
        Self {
            name: name.into(),
            description: description.into(),
            parameters_schema: serde_json::json!({
                "type": "object",
                "properties": {}
            }),
            handler,
            metadata: ToolMetadata::default(),
        }
    }

    /// 设置参数 Schema
    /// Set parameters Schema
    pub fn with_schema(mut self, schema: serde_json::Value) -> Self {
        self.parameters_schema = schema;
        self
    }

    /// 设置元数据
    /// Set metadata
    pub fn with_metadata(mut self, metadata: ToolMetadata) -> Self {
        self.metadata = metadata;
        self
    }
}

#[async_trait]
impl<F> Tool for ClosureTool<F>
where
    F: Fn(ToolInput) -> ToolResult + Send + Sync,
{
    fn name(&self) -> &str {
        &self.name
    }

    fn description(&self) -> &str {
        &self.description
    }

    fn parameters_schema(&self) -> serde_json::Value {
        self.parameters_schema.clone()
    }

    async fn execute(&self, input: ToolInput, _ctx: &AgentContext) -> ToolResult {
        (self.handler)(input)
    }

    fn metadata(&self) -> ToolMetadata {
        self.metadata.clone()
    }
}

// ============================================================================
// 便捷工具创建宏
// Macro for convenient tool creation
// ============================================================================

/// 创建简单同步工具
/// Create simple synchronous tools
#[macro_export]
macro_rules! simple_tool {
    ($name:expr, $desc:expr, $handler:expr) => {
        $crate::agent::tools::ClosureTool::new($name, $desc, $handler)
    };
    ($name:expr, $desc:expr, $schema:expr, $handler:expr) => {
        $crate::agent::tools::ClosureTool::new($name, $desc, $handler).with_schema($schema)
    };
}

// ============================================================================
// 内置工具集合
// Built-in tools collection
// ============================================================================

/// 内置工具集合
/// Built-in tools collection
pub struct BuiltinTools;

impl BuiltinTools {
    /// 创建计算器工具
    /// Create a calculator tool
    pub fn calculator() -> impl Tool {
        ClosureTool::new(
            "calculator",
            "Perform basic arithmetic operations",
            |input| {
                let operation = input.get_str("operation").unwrap_or("add");
                let a = input.get_number("a").unwrap_or(0.0);
                let b = input.get_number("b").unwrap_or(0.0);

                let result = match operation {
                    "add" => a + b,
                    "sub" => a - b,
                    "mul" => a * b,
                    "div" => {
                        if b == 0.0 {
                            return ToolResult::failure("Division by zero");
                        }
                        a / b
                    }
                    _ => return ToolResult::failure(format!("Unknown operation: {}", operation)),
                };

                ToolResult::success_text(format!("{}", result))
            },
        )
        .with_schema(serde_json::json!({
            "type": "object",
            "properties": {
                "operation": {
                    "type": "string",
                    "enum": ["add", "sub", "mul", "div"],
                    "description": "The arithmetic operation to perform"
                },
                "a": {
                    "type": "number",
                    "description": "First operand"
                },
                "b": {
                    "type": "number",
                    "description": "Second operand"
                }
            },
            "required": ["operation", "a", "b"]
        }))
    }

    /// 创建当前时间工具
    /// Create a current time tool
    pub fn current_time() -> impl Tool {
        ClosureTool::new("current_time", "Get the current date and time", |_input| {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();

            ToolResult::success(serde_json::json!({
                "timestamp": now,
                "formatted": format!("Unix timestamp: {}", now)
            }))
        })
    }

    /// 创建 JSON 解析工具
    /// Create a JSON parser tool
    pub fn json_parser() -> impl Tool {
        ClosureTool::new(
            "json_parser",
            "Parse JSON string into structured data",
            |input| {
                let json_str = match input.get_str("json") {
                    Some(s) => s,
                    None => return ToolResult::failure("No JSON string provided"),
                };

                match serde_json::from_str::<serde_json::Value>(json_str) {
                    Ok(parsed) => ToolResult::success(parsed),
                    Err(e) => ToolResult::failure(format!("Failed to parse JSON: {}", e)),
                }
            },
        )
        .with_schema(serde_json::json!({
            "type": "object",
            "properties": {
                "json": {
                    "type": "string",
                    "description": "The JSON string to parse"
                }
            },
            "required": ["json"]
        }))
    }

    /// 创建字符串处理工具
    /// Create a string processing tool
    pub fn string_utils() -> impl Tool {
        ClosureTool::new("string_utils", "String manipulation utilities", |input| {
            let operation = input.get_str("operation").unwrap_or("length");
            let text = input.get_str("text").unwrap_or("");

            let result = match operation {
                "length" => serde_json::json!({ "length": text.len() }),
                "upper" => serde_json::json!({ "result": text.to_uppercase() }),
                "lower" => serde_json::json!({ "result": text.to_lowercase() }),
                "trim" => serde_json::json!({ "result": text.trim() }),
                "reverse" => {
                    serde_json::json!({ "result": text.chars().rev().collect::<String>() })
                }
                "word_count" => serde_json::json!({ "count": text.split_whitespace().count() }),
                _ => return ToolResult::failure(format!("Unknown operation: {}", operation)),
            };

            ToolResult::success(result)
        })
        .with_schema(serde_json::json!({
            "type": "object",
            "properties": {
                "operation": {
                    "type": "string",
                    "enum": ["length", "upper", "lower", "trim", "reverse", "word_count"],
                    "description": "The string operation to perform"
                },
                "text": {
                    "type": "string",
                    "description": "The text to process"
                }
            },
            "required": ["operation", "text"]
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_closure_tool() {
        let tool = ClosureTool::new("test", "Test tool", |input| {
            let msg = input.get_str("message").unwrap_or("default");
            ToolResult::success_text(format!("Got: {}", msg))
        });

        let ctx = AgentContext::new("test");
        let input = ToolInput::from_json(serde_json::json!({"message": "hello"}));

        let result = tool.execute(input, &ctx).await;
        assert!(result.success);
        assert_eq!(result.as_text(), Some("Got: hello"));
    }

    #[tokio::test]
    async fn test_calculator_tool() {
        let tool = BuiltinTools::calculator();
        let ctx = AgentContext::new("test");

        // Test addition
        // Test addition
        let input = ToolInput::from_json(serde_json::json!({
            "operation": "add",
            "a": 5,
            "b": 3
        }));
        let result = tool.execute(input, &ctx).await;
        assert!(result.success);
        assert_eq!(result.as_text(), Some("8"));

        // Test division by zero
        // Test division by zero
        let input = ToolInput::from_json(serde_json::json!({
            "operation": "div",
            "a": 10,
            "b": 0
        }));
        let result = tool.execute(input, &ctx).await;
        assert!(!result.success);
    }

    #[tokio::test]
    async fn test_string_utils_tool() {
        let tool = BuiltinTools::string_utils();
        let ctx = AgentContext::new("test");

        let input = ToolInput::from_json(serde_json::json!({
            "operation": "upper",
            "text": "hello world"
        }));
        let result = tool.execute(input, &ctx).await;
        assert!(result.success);

        let output = result.output;
        assert_eq!(output["result"], "HELLO WORLD");
    }
}
