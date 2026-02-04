//! 工具组件
//!
//! 从 kernel 层导入 Tool trait，提供具体实现和扩展

use mofa_kernel::agent::components::tool::{
    LLMTool, ToolDescriptor, ToolInput, ToolMetadata, ToolRegistry, ToolResult,
};
use mofa_kernel::agent::context::AgentContext;
use mofa_kernel::agent::error::{AgentError, AgentResult};
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

    async fn execute(&self, input: ToolInput, _ctx: &AgentContext) -> ToolResult {
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
        let ctx = AgentContext::new("test");
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

        let ctx = AgentContext::new("test");
        let result = registry
            .execute("echo", ToolInput::from_json(json!({"message": "test"})), &ctx)
            .await
            .unwrap();

        assert!(result.success);
    }
}
