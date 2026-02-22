//! LLM tool executor adapters for the SDK.

use mofa_foundation::llm::{LLMError, LLMResult, Tool, ToolExecutor};
use mofa_plugins::{ToolCall, ToolDefinition, ToolPlugin};
use std::sync::Arc;
use tokio::sync::RwLock;

/// Adapter that exposes a ToolPlugin as an LLM ToolExecutor.
///
/// This lets LLM agents discover tools directly from a ToolPlugin without
/// manually constructing tool definitions.
pub struct ToolPluginExecutor {
    tool_plugin: Arc<RwLock<ToolPlugin>>,
    cached_tools: Arc<RwLock<Option<Vec<Tool>>>>,
}

impl ToolPluginExecutor {
    /// Create a new adapter from an owned ToolPlugin.
    pub fn new(tool_plugin: ToolPlugin) -> Self {
        Self {
            tool_plugin: Arc::new(RwLock::new(tool_plugin)),
            cached_tools: Arc::new(RwLock::new(None)),
        }
    }

    /// Create a new adapter from a shared ToolPlugin handle.
    pub fn with_shared(tool_plugin: Arc<RwLock<ToolPlugin>>) -> Self {
        Self {
            tool_plugin,
            cached_tools: Arc::new(RwLock::new(None)),
        }
    }

    /// Get the underlying ToolPlugin handle.
    pub fn tool_plugin(&self) -> Arc<RwLock<ToolPlugin>> {
        self.tool_plugin.clone()
    }

    /// Clear the cached tool list so it will be refreshed on next access.
    pub async fn invalidate_cache(&self) {
        let mut cache = self.cached_tools.write().await;
        *cache = None;
    }

    /// Refresh and return the latest tool list from the plugin.
    pub async fn refresh_tools(&self) -> LLMResult<Vec<Tool>> {
        let plugin = self.tool_plugin.read().await;
        let tools = plugin
            .list_tools()
            .into_iter()
            .map(|def| Self::definition_to_tool(&def))
            .collect::<Vec<_>>();

        let mut cache = self.cached_tools.write().await;
        *cache = Some(tools.clone());
        Ok(tools)
    }

    fn definition_to_tool(def: &ToolDefinition) -> Tool {
        Tool::function(&def.name, &def.description, def.parameters.clone())
    }
}

#[async_trait::async_trait]
impl ToolExecutor for ToolPluginExecutor {
    async fn execute(&self, name: &str, arguments: &str) -> LLMResult<String> {
        let args: serde_json::Value = serde_json::from_str(arguments)
            .map_err(|e| LLMError::Other(format!("参数解析失败: {}", e)))?;

        let call = ToolCall {
            call_id: uuid::Uuid::now_v7().to_string(),
            name: name.to_string(),
            arguments: args,
        };

        let mut plugin = self.tool_plugin.write().await;
        let result = plugin
            .call_tool(call)
            .await
            .map_err(|e| LLMError::Other(format!("工具执行失败: {}", e)))?;

        if result.success {
            serde_json::to_string(&result.result)
                .map_err(|e| LLMError::Other(format!("结果序列化失败: {}", e)))
        } else {
            Err(LLMError::Other(
                result.error.unwrap_or_else(|| "未知错误".to_string()),
            ))
        }
    }

    async fn available_tools(&self) -> LLMResult<Vec<Tool>> {
        let cached = { self.cached_tools.read().await.clone() };
        if let Some(tools) = cached {
            return Ok(tools);
        }

        self.refresh_tools().await
    }
}
