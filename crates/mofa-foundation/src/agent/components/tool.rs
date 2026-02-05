//! 工具组件
//!
//! 从 kernel 层导入 Tool trait，提供具体实现和扩展

use mofa_kernel::agent::components::tool::{
    ToolDescriptor, ToolInput, ToolMetadata, ToolRegistry, ToolResult,
};
use mofa_kernel::agent::context::CoreAgentContext;
use mofa_kernel::agent::error::AgentResult;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::any::Any;
use std::collections::HashMap;
use std::sync::Arc;

// ============================================================================
// Foundation 层扩展类型
// ============================================================================

/// Tool categories for organization and discovery
///
/// Foundation-specific extension for tool categorization
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ToolCategory {
    /// File operations (read, write, edit)
    File,
    /// Command execution (shell, scripts)
    Shell,
    /// Web operations (search, fetch)
    Web,
    /// Memory operations (read, write memory)
    Memory,
    /// Agent control (spawn, coordinate)
    Agent,
    /// Messaging and communication
    Communication,
    /// General purpose tools
    General,
    /// Custom tools
    Custom,
}

impl ToolCategory {
    /// Get the category as a string
    pub fn as_str(&self) -> &str {
        match self {
            Self::File => "file",
            Self::Shell => "shell",
            Self::Web => "web",
            Self::Memory => "memory",
            Self::Agent => "agent",
            Self::Communication => "communication",
            Self::General => "general",
            Self::Custom => "custom",
        }
    }

    /// Parse from string
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "file" => Some(Self::File),
            "shell" => Some(Self::Shell),
            "web" => Some(Self::Web),
            "memory" => Some(Self::Memory),
            "agent" => Some(Self::Agent),
            "communication" => Some(Self::Communication),
            "general" => Some(Self::General),
            "custom" => Some(Self::Custom),
            _ => None,
        }
    }
}

/// 扩展的 Tool trait (Foundation 特有方法)
///
/// 注意：这是 Foundation 层提供的扩展 trait，不是 kernel 层的 Tool trait
pub trait ToolExt: mofa_kernel::agent::components::tool::Tool {
    /// 工具分类
    fn category(&self) -> ToolCategory;

    /// 转换为 OpenAI function schema 格式 (兼容性方法)
    fn to_openai_schema(&self) -> Value {
        use mofa_kernel::agent::components::tool::Tool;
        json!({
            "type": "function",
            "function": {
                "name": self.name(),
                "description": self.description(),
                "parameters": self.parameters_schema()
            }
        })
    }

    /// Get this tool as `Any` for downcasting
    fn as_any(&self) -> &dyn Any;
}

// ============================================================================
// SimpleTool - Convenience trait for tools that don't need context
// ============================================================================

/// Simple tool trait for tools that don't need AgentContext
///
/// This is a convenience trait for implementing simple tools that only need
/// the input parameters and don't require access to the agent context.
/// SimpleToolAdapter automatically implements the full Tool trait.
///
/// # Example
///
/// ```rust,ignore
/// use mofa_foundation::agent::components::tool::{SimpleTool, ToolInput, ToolResult};
/// use serde_json::json;
///
/// struct HelloTool;
///
/// impl SimpleTool for HelloTool {
///     fn name(&self) -> &str {
///         "hello"
///     }
///
///     fn description(&self) -> &str {
///         "Says hello to someone"
///     }
///
///     fn parameters_schema(&self) -> serde_json::Value {
///         json!({
///             "type": "object",
///             "properties": {
///                 "name": {"type": "string"}
///             },
///             "required": ["name"]
///         })
///     }
///
///     async fn execute(&self, input: ToolInput) -> ToolResult {
///         let name = input.get_str("name").unwrap_or("World");
///         ToolResult::success_text(format!("Hello, {}!", name))
///     }
/// }
/// ```
#[async_trait]
pub trait SimpleTool: Send + Sync {
    /// Get the tool's name
    fn name(&self) -> &str;

    /// Get the tool's description
    fn description(&self) -> &str;

    /// Get the JSON Schema for the tool's parameters
    fn parameters_schema(&self) -> Value;

    /// Execute the tool with given input (no context needed)
    async fn execute(&self, input: ToolInput) -> ToolResult;

    /// Get tool metadata (optional override)
    fn metadata(&self) -> ToolMetadata {
        ToolMetadata::default()
    }

    /// Get tool category (optional override)
    fn category(&self) -> ToolCategory {
        ToolCategory::Custom
    }
}

/// Adapter that implements the full Tool trait for SimpleTool
///
/// This adapter wraps a SimpleTool and implements the Tool trait by
/// ignoring the AgentContext parameter.
pub struct SimpleToolAdapter<T: SimpleTool> {
    inner: T,
}

impl<T: SimpleTool> SimpleToolAdapter<T> {
    /// Create a new adapter from a SimpleTool
    pub fn new(inner: T) -> Self {
        Self { inner }
    }

    /// Get a reference to the inner tool
    pub fn inner(&self) -> &T {
        &self.inner
    }
}

#[async_trait]
impl<T: SimpleTool + Send + Sync + 'static> mofa_kernel::agent::components::tool::Tool for SimpleToolAdapter<T> {
    fn name(&self) -> &str {
        self.inner.name()
    }

    fn description(&self) -> &str {
        self.inner.description()
    }

    fn parameters_schema(&self) -> Value {
        self.inner.parameters_schema()
    }

    async fn execute(&self, input: ToolInput, _ctx: &CoreAgentContext) -> ToolResult {
        self.inner.execute(input).await
    }

    fn metadata(&self) -> ToolMetadata {
        self.inner.metadata()
    }
}

impl<T: SimpleTool + 'static> ToolExt for SimpleToolAdapter<T> {
    fn category(&self) -> ToolCategory {
        self.inner.category()
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// Convenience function to convert a SimpleTool into an Arc<dyn Tool>
///
/// # Example
///
/// ```rust,ignore
/// use mofa_foundation::agent::components::tool::{SimpleTool, as_tool};
/// use std::sync::Arc;
///
/// let tool = Arc::new(MySimpleTool);
/// let tool_ref = as_tool(tool);
/// registry.register(tool_ref)?;
/// ```
pub fn as_tool<T: SimpleTool + Send + Sync + 'static>(tool: T) -> Arc<dyn mofa_kernel::agent::components::tool::Tool> {
    Arc::new(SimpleToolAdapter::new(tool))
}

// ============================================================================
// 工具注册中心实现
// ============================================================================

/// 简单工具注册中心实现
///
/// Foundation 层的具体实现
pub struct SimpleToolRegistry {
    tools: HashMap<String, Arc<dyn mofa_kernel::agent::components::tool::Tool>>,
}

impl SimpleToolRegistry {
    /// 创建新的注册中心
    pub fn new() -> Self {
        Self {
            tools: HashMap::new(),
        }
    }
}

impl Default for SimpleToolRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl ToolRegistry for SimpleToolRegistry {
    fn register(&mut self, tool: Arc<dyn mofa_kernel::agent::components::tool::Tool>) -> AgentResult<()> {
        self.tools.insert(tool.name().to_string(), tool);
        Ok(())
    }

    fn get(&self, name: &str) -> Option<Arc<dyn mofa_kernel::agent::components::tool::Tool>> {
        self.tools.get(name).cloned()
    }

    fn unregister(&mut self, name: &str) -> AgentResult<bool> {
        Ok(self.tools.remove(name).is_some())
    }

    fn list(&self) -> Vec<ToolDescriptor> {
        self.tools
            .values()
            .map(|t| ToolDescriptor::from_tool(t.as_ref()))
            .collect()
    }

    fn list_names(&self) -> Vec<String> {
        self.tools.keys().cloned().collect()
    }

    fn contains(&self, name: &str) -> bool {
        self.tools.contains_key(name)
    }

    fn count(&self) -> usize {
        self.tools.len()
    }
}

// ============================================================================
// 内置工具
// ============================================================================

/// Echo 工具 (用于测试)
pub struct EchoTool;

#[async_trait]
impl mofa_kernel::agent::components::tool::Tool for EchoTool {
    fn name(&self) -> &str {
        "echo"
    }

    fn description(&self) -> &str {
        "Echo the input back as output"
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "message": {
                    "type": "string",
                    "description": "The message to echo"
                }
            },
            "required": ["message"]
        })
    }

    async fn execute(&self, input: ToolInput, _ctx: &CoreAgentContext) -> ToolResult {
        if let Some(message) = input.get_str("message") {
            ToolResult::success_text(message)
        } else if let Some(raw) = &input.raw_input {
            ToolResult::success_text(raw)
        } else {
            ToolResult::failure("No message provided")
        }
    }
}

impl ToolExt for EchoTool {
    fn category(&self) -> ToolCategory {
        ToolCategory::General
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mofa_kernel::agent::components::tool::Tool; // Import Tool trait for method resolution

    #[tokio::test]
    async fn test_echo_tool() {
        let tool = EchoTool;
        let ctx = CoreAgentContext::new("test");
        let input = ToolInput::from_json(json!({"message": "Hello!"}));

        let result = tool.execute(input, &ctx).await;
        assert!(result.success);
        assert_eq!(result.as_text(), Some("Hello!"));
    }

    #[test]
    fn test_tool_category() {
        let category = ToolCategory::File;
        assert_eq!(category.as_str(), "file");
        assert_eq!(ToolCategory::from_str("file"), Some(ToolCategory::File));
    }

    #[test]
    fn test_tool_ext() {
        let tool = EchoTool;
        assert_eq!(tool.category(), ToolCategory::General);
        let schema = tool.to_openai_schema();
        assert_eq!(schema["function"]["name"], "echo");
    }

    #[tokio::test]
    async fn test_simple_tool_registry() {
        let mut registry = SimpleToolRegistry::new();
        registry.register(Arc::new(EchoTool)).unwrap();

        assert!(registry.contains("echo"));
        assert_eq!(registry.count(), 1);

        let ctx = CoreAgentContext::new("test");
        let result = registry
            .execute("echo", ToolInput::from_json(json!({"message": "test"})), &ctx)
            .await
            .unwrap();

        assert!(result.success);
    }

    // SimpleTool tests
    struct TestSimpleTool {
        name: String,
    }

    #[async_trait]
    impl SimpleTool for TestSimpleTool {
        fn name(&self) -> &str {
            &self.name
        }

        fn description(&self) -> &str {
            "A test tool"
        }

        fn parameters_schema(&self) -> Value {
            json!({
                "type": "object",
                "properties": {
                    "value": {"type": "string"}
                }
            })
        }

        async fn execute(&self, input: ToolInput) -> ToolResult {
            if let Some(value) = input.get_str("value") {
                ToolResult::success_text(format!("Got: {}", value))
            } else {
                ToolResult::failure("No value provided")
            }
        }

        fn category(&self) -> ToolCategory {
            ToolCategory::Custom
        }
    }

    #[tokio::test]
    async fn test_simple_tool() {
        let tool = TestSimpleTool { name: "test_tool".to_string() };
        let input = ToolInput::from_json(json!({"value": "hello"}));

        let result = tool.execute(input).await;
        assert!(result.success);
        assert_eq!(result.as_text(), Some("Got: hello"));
    }

    #[tokio::test]
    async fn test_simple_tool_adapter() {
        let simple_tool = TestSimpleTool { name: "test_adapter".to_string() };
        let adapter = SimpleToolAdapter::new(simple_tool);

        assert_eq!(adapter.name(), "test_adapter");
        assert_eq!(adapter.description(), "A test tool");
        assert_eq!(adapter.category(), ToolCategory::Custom);

        let ctx = CoreAgentContext::new("test");
        let input = ToolInput::from_json(json!({"value": "world"}));

        let result = mofa_kernel::agent::components::tool::Tool::execute(&adapter, input, &ctx).await;
        assert!(result.success);
        assert_eq!(result.as_text(), Some("Got: world"));
    }

    #[tokio::test]
    async fn test_as_tool_function() {
        let simple_tool = TestSimpleTool { name: "test_as_tool".to_string() };
        let tool_ref = as_tool(simple_tool);

        let mut registry = SimpleToolRegistry::new();
        registry.register(tool_ref).unwrap();

        assert!(registry.contains("test_as_tool"));

        let ctx = CoreAgentContext::new("test");
        let result = registry
            .execute("test_as_tool", ToolInput::from_json(json!({"value": "test"})), &ctx)
            .await
            .unwrap();

        assert!(result.success);
    }
}
