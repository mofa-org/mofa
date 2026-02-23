//! 工具组件
//! Tool Component
//!
//! 定义统一的工具接口，合并 ToolExecutor 和 ReActTool
//! Defines a unified tool interface, merging ToolExecutor and ReActTool

use crate::agent::context::AgentContext;
use crate::agent::error::{AgentError, AgentResult};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;

/// 统一工具 Trait
/// Unified Tool Trait
///
/// 合并了 ToolExecutor 和 ReActTool 的功能
/// Merges functionalities of ToolExecutor and ReActTool
///
/// # 示例
/// # Example
///
/// ```rust,ignore
/// use mofa_kernel::agent::components::tool::{Tool, ToolInput, ToolResult, ToolMetadata};
///
/// struct Calculator;
///
/// #[async_trait]
/// impl Tool for Calculator {
///     fn name(&self) -> &str { "calculator" }
///     fn description(&self) -> &str { "Perform arithmetic operations" }
///     fn parameters_schema(&self) -> serde_json::Value {
///         serde_json::json!({
///             "type": "object",
///             "properties": {
///                 "operation": { "type": "string", "enum": ["add", "sub", "mul", "div"] },
///                 "a": { "type": "number" },
///                 "b": { "type": "number" }
///             },
///             "required": ["operation", "a", "b"]
///         })
///     }
///
///     async fn execute(&self, input: ToolInput, ctx: &CoreAgentContext) -> ToolResult {
///         // Implementation
///     }
///
///     fn metadata(&self) -> ToolMetadata {
///         ToolMetadata::default()
///     }
/// }
/// ```
#[async_trait]
pub trait Tool: Send + Sync {
    /// 工具名称 (唯一标识符)
    /// Tool name (unique identifier)
    fn name(&self) -> &str;

    /// 工具描述 (用于 LLM 理解)
    /// Tool description (for LLM understanding)
    fn description(&self) -> &str;

    /// 参数 JSON Schema
    /// Parameters JSON Schema
    fn parameters_schema(&self) -> serde_json::Value;

    /// 执行工具
    /// Execute tool
    async fn execute(&self, input: ToolInput, ctx: &AgentContext) -> ToolResult;

    /// 工具元数据
    /// Tool metadata
    fn metadata(&self) -> ToolMetadata {
        ToolMetadata::default()
    }

    /// 验证输入
    /// Validate input
    fn validate_input(&self, input: &ToolInput) -> AgentResult<()> {
        // 默认不做验证，子类可以覆盖
        // No validation by default, subclasses can override
        let _ = input;
        Ok(())
    }

    /// 是否需要确认
    /// Whether confirmation is required
    fn requires_confirmation(&self) -> bool {
        false
    }

    /// 转换为 LLM Tool 格式
    /// Convert to LLM Tool format
    fn to_llm_tool(&self) -> LLMTool {
        LLMTool {
            name: self.name().to_string(),
            description: self.description().to_string(),
            parameters: self.parameters_schema(),
        }
    }
}

/// 工具输入
/// Tool Input
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolInput {
    /// 结构化参数
    /// Structured arguments
    pub arguments: serde_json::Value,
    /// 原始输入 (可选)
    /// Raw input (optional)
    pub raw_input: Option<String>,
}

impl ToolInput {
    /// 从 JSON 参数创建
    /// Create from JSON arguments
    pub fn from_json(arguments: serde_json::Value) -> Self {
        Self {
            arguments,
            raw_input: None,
        }
    }

    /// 从原始字符串创建
    /// Create from raw string
    pub fn from_raw(raw: impl Into<String>) -> Self {
        let raw = raw.into();
        Self {
            arguments: serde_json::Value::String(raw.clone()),
            raw_input: Some(raw),
        }
    }

    /// 获取参数值
    /// Get parameter value
    pub fn get<T: serde::de::DeserializeOwned>(&self, key: &str) -> Option<T> {
        self.arguments
            .get(key)
            .and_then(|v| serde_json::from_value(v.clone()).ok())
    }

    /// 获取字符串参数
    /// Get string parameter
    pub fn get_str(&self, key: &str) -> Option<&str> {
        self.arguments.get(key).and_then(|v| v.as_str())
    }

    /// 获取数字参数
    /// Get number parameter
    pub fn get_number(&self, key: &str) -> Option<f64> {
        self.arguments.get(key).and_then(|v| v.as_f64())
    }

    /// 获取布尔参数
    /// Get boolean parameter
    pub fn get_bool(&self, key: &str) -> Option<bool> {
        self.arguments.get(key).and_then(|v| v.as_bool())
    }
}

impl From<serde_json::Value> for ToolInput {
    fn from(v: serde_json::Value) -> Self {
        Self::from_json(v)
    }
}

impl From<String> for ToolInput {
    fn from(s: String) -> Self {
        Self::from_raw(s)
    }
}

impl From<&str> for ToolInput {
    fn from(s: &str) -> Self {
        Self::from_raw(s)
    }
}

/// 工具执行结果
/// Tool Execution Result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolResult {
    /// 是否成功
    /// Whether successful
    pub success: bool,
    /// 输出内容
    /// Output content
    pub output: serde_json::Value,
    /// 错误信息 (如果失败)
    /// Error message (if failed)
    pub error: Option<String>,
    /// 额外元数据
    /// Additional metadata
    pub metadata: HashMap<String, String>,
}

impl ToolResult {
    /// 创建成功结果
    /// Create success result
    pub fn success(output: serde_json::Value) -> Self {
        Self {
            success: true,
            output,
            error: None,
            metadata: HashMap::new(),
        }
    }

    /// 创建文本成功结果
    /// Create text success result
    pub fn success_text(text: impl Into<String>) -> Self {
        Self::success(serde_json::Value::String(text.into()))
    }

    /// 创建失败结果
    /// Create failure result
    pub fn failure(error: impl Into<String>) -> Self {
        Self {
            success: false,
            output: serde_json::Value::Null,
            error: Some(error.into()),
            metadata: HashMap::new(),
        }
    }

    /// 添加元数据
    /// Add metadata
    pub fn with_metadata(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.metadata.insert(key.into(), value.into());
        self
    }

    /// 获取文本输出
    /// Get text output
    pub fn as_text(&self) -> Option<&str> {
        self.output.as_str()
    }

    /// 转换为字符串
    /// Convert to string output
    pub fn to_string_output(&self) -> String {
        if self.success {
            match &self.output {
                serde_json::Value::String(s) => s.clone(),
                v => v.to_string(),
            }
        } else {
            format!(
                "Error: {}",
                self.error.as_deref().unwrap_or("Unknown error")
            )
        }
    }
}

/// 工具元数据
/// Tool Metadata
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ToolMetadata {
    /// 工具分类
    /// Tool category
    pub category: Option<String>,
    /// 工具标签
    /// Tool tags
    pub tags: Vec<String>,
    /// 是否为危险操作
    /// Whether it is a dangerous operation
    pub is_dangerous: bool,
    /// 是否需要网络
    /// Whether network is required
    pub requires_network: bool,
    /// 是否需要文件系统访问
    /// Whether filesystem access is required
    pub requires_filesystem: bool,
    /// 自定义属性
    /// Custom attributes
    pub custom: HashMap<String, serde_json::Value>,
}

impl ToolMetadata {
    /// 创建新的元数据
    /// Create new metadata
    pub fn new() -> Self {
        Self::default()
    }

    /// 设置分类
    /// Set category
    pub fn with_category(mut self, category: impl Into<String>) -> Self {
        self.category = Some(category.into());
        self
    }

    /// 添加标签
    /// Add tag
    pub fn with_tag(mut self, tag: impl Into<String>) -> Self {
        self.tags.push(tag.into());
        self
    }

    /// 标记为危险操作
    /// Mark as dangerous operation
    pub fn dangerous(mut self) -> Self {
        self.is_dangerous = true;
        self
    }

    /// 标记需要网络
    /// Mark as requiring network
    pub fn needs_network(mut self) -> Self {
        self.requires_network = true;
        self
    }

    /// 标记需要文件系统
    /// Mark as requiring filesystem
    pub fn needs_filesystem(mut self) -> Self {
        self.requires_filesystem = true;
        self
    }
}

/// 工具描述符 (用于列表展示)
/// Tool Descriptor (for list display)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDescriptor {
    /// 工具名称
    /// Tool name
    pub name: String,
    /// 工具描述
    /// Tool description
    pub description: String,
    /// 参数 Schema
    /// Parameters Schema
    pub parameters_schema: serde_json::Value,
    /// 元数据
    /// Metadata
    pub metadata: ToolMetadata,
}

impl ToolDescriptor {
    /// 从 Tool 创建描述符
    /// Create descriptor from Tool
    pub fn from_tool(tool: &dyn Tool) -> Self {
        Self {
            name: tool.name().to_string(),
            description: tool.description().to_string(),
            parameters_schema: tool.parameters_schema(),
            metadata: tool.metadata(),
        }
    }
}

/// LLM Tool 格式 (用于 API 调用)
/// LLM Tool format (for API calls)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LLMTool {
    /// 工具名称
    /// Tool name
    pub name: String,
    /// 工具描述
    /// Tool description
    pub description: String,
    /// 参数 Schema
    /// Parameters Schema
    pub parameters: serde_json::Value,
}

// ============================================================================
// 工具注册中心 Trait (接口仅在此定义)
// Tool Registry Trait (Interface defined here only)
// ============================================================================

/// 定义工具注册的接口，具体实现在 foundation 层。
/// Defines the tool registration interface, with concrete implementation in the foundation layer.
#[async_trait]
pub trait ToolRegistry: Send + Sync {
    /// 注册工具
    /// Register tool
    fn register(&mut self, tool: Arc<dyn Tool>) -> AgentResult<()>;

    /// 批量注册工具
    /// Batch register tools
    fn register_all(&mut self, tools: Vec<Arc<dyn Tool>>) -> AgentResult<()> {
        for tool in tools {
            self.register(tool)?;
        }
        Ok(())
    }

    /// 获取工具
    /// Get tool
    fn get(&self, name: &str) -> Option<Arc<dyn Tool>>;

    /// 移除工具
    /// Remove tool
    fn unregister(&mut self, name: &str) -> AgentResult<bool>;

    /// 列出所有工具
    /// List all tools
    fn list(&self) -> Vec<ToolDescriptor>;

    /// 列出所有工具名称
    /// List all tool names
    fn list_names(&self) -> Vec<String>;

    /// 检查工具是否存在
    /// Check if tool exists
    fn contains(&self, name: &str) -> bool;

    /// 获取工具数量
    /// Get tool count
    fn count(&self) -> usize;

    /// 执行工具
    /// Execute tool
    async fn execute(
        &self,
        name: &str,
        input: ToolInput,
        ctx: &AgentContext,
    ) -> AgentResult<ToolResult> {
        let tool = self
            .get(name)
            .ok_or_else(|| AgentError::ToolNotFound(name.to_string()))?;
        tool.validate_input(&input)?;
        Ok(tool.execute(input, ctx).await)
    }

    /// 转换为 LLM Tools
    /// Convert to LLM Tools
    fn to_llm_tools(&self) -> Vec<LLMTool> {
        self.list()
            .iter()
            .map(|d| LLMTool {
                name: d.name.clone(),
                description: d.description.clone(),
                parameters: d.parameters_schema.clone(),
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::context::AgentContext;

    #[test]
    fn test_tool_input_from_json() {
        let input = ToolInput::from_json(serde_json::json!({
            "name": "test",
            "count": 42
        }));

        assert_eq!(input.get_str("name"), Some("test"));
        assert_eq!(input.get_number("count"), Some(42.0));
    }

    #[test]
    fn test_tool_result() {
        let success = ToolResult::success_text("OK");
        assert!(success.success);
        assert_eq!(success.as_text(), Some("OK"));

        let failure = ToolResult::failure("Something went wrong");
        assert!(!failure.success);
        assert!(failure.error.is_some());
    }
}
