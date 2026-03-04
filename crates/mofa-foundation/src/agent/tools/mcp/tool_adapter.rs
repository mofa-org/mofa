//! MCP 工具适配器
//! MCP Tool Adapter
//!
//! 将 MCP 服务器上的工具包装为内核 `Tool` trait 实现。
//! Wrap tools from MCP servers as kernel `Tool` trait implementations.

use async_trait::async_trait;
use mofa_kernel::agent::components::mcp::McpToolInfo;
use mofa_kernel::agent::components::tool::{Tool, ToolInput, ToolMetadata, ToolResult};
use mofa_kernel::agent::context::AgentContext;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

use super::McpClientManager;

/// MCP 工具适配器
/// MCP Tool Adapter
///
/// 将 MCP 服务器上的单个工具包装为内核 `Tool` trait。
/// Wraps a single tool on an MCP server into the kernel `Tool` trait.
/// 当 Agent 调用此工具时，它会通过 `McpClientManager` 转发到 MCP 服务器。
/// When the Agent calls this tool, it is forwarded to the MCP server via `McpClientManager`.
///
/// # 示例
/// # Example
///
/// ```rust,ignore
/// let adapter = McpToolAdapter::new(
///     "github",
///     tool_info,
///     client_manager.clone(),
/// );
///
/// // 作为普通 Tool 使用
/// // Use as a standard Tool
/// let result = adapter.execute(input, &ctx).await;
/// ```
pub struct McpToolAdapter {
    /// MCP 服务器名称
    /// MCP server name
    server_name: String,
    /// 工具元信息
    /// Tool metadata info
    tool_info: McpToolInfo,
    /// MCP 客户端管理器 (共享引用)
    /// MCP client manager (shared reference)
    client: Arc<RwLock<McpClientManager>>,
}

impl McpToolAdapter {
    /// 创建新的 MCP 工具适配器
    /// Create a new MCP tool adapter
    pub fn new(
        server_name: impl Into<String>,
        tool_info: McpToolInfo,
        client: Arc<RwLock<McpClientManager>>,
    ) -> Self {
        Self {
            server_name: server_name.into(),
            tool_info,
            client,
        }
    }

    /// 获取服务器名称
    /// Get server name
    pub fn server_name(&self) -> &str {
        &self.server_name
    }
}

#[async_trait]
impl Tool for McpToolAdapter {
    fn name(&self) -> &str {
        &self.tool_info.name
    }

    fn description(&self) -> &str {
        &self.tool_info.description
    }

    fn parameters_schema(&self) -> serde_json::Value {
        self.tool_info.input_schema.clone()
    }

    async fn execute(&self, input: ToolInput, _ctx: &AgentContext) -> ToolResult {
        use mofa_kernel::agent::components::mcp::McpClient;

        let client = self.client.read().await;
        match client
            .call_tool(&self.server_name, &self.tool_info.name, input.arguments)
            .await
        {
            Ok(output) => ToolResult::success(output),
            Err(e) => ToolResult::failure(format!("MCP tool call failed: {}", e)),
        }
    }

    fn metadata(&self) -> ToolMetadata {
        let mut custom = HashMap::new();
        custom.insert(
            "mcp_server".to_string(),
            serde_json::Value::String(self.server_name.clone()),
        );

        ToolMetadata {
            category: Some("mcp".to_string()),
            tags: vec!["mcp".to_string(), self.server_name.clone()],
            is_dangerous: false,
            requires_network: true,
            requires_filesystem: false,
            custom,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mofa_kernel::agent::components::mcp::McpToolInfo;

    #[test]
    fn test_mcp_tool_adapter_metadata() {
        let tool_info = McpToolInfo {
            name: "list_repos".to_string(),
            description: "List GitHub repositories".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "owner": { "type": "string" }
                }
            }),
        };

        let client = Arc::new(RwLock::new(McpClientManager::new()));
        let adapter = McpToolAdapter::new("github", tool_info, client);

        assert_eq!(adapter.name(), "list_repos");
        assert_eq!(adapter.description(), "List GitHub repositories");
        assert_eq!(adapter.server_name(), "github");

        let metadata = adapter.metadata();
        assert_eq!(metadata.category, Some("mcp".to_string()));
        assert!(metadata.requires_network);
        assert!(metadata.tags.contains(&"mcp".to_string()));
        assert!(metadata.tags.contains(&"github".to_string()));
    }

    #[test]
    fn test_mcp_tool_adapter_schema() {
        let schema = serde_json::json!({
            "type": "object",
            "properties": {
                "query": { "type": "string" }
            },
            "required": ["query"]
        });

        let tool_info = McpToolInfo {
            name: "search".to_string(),
            description: "Search for items".to_string(),
            input_schema: schema.clone(),
        };

        let client = Arc::new(RwLock::new(McpClientManager::new()));
        let adapter = McpToolAdapter::new("search-server", tool_info, client);

        assert_eq!(adapter.parameters_schema(), schema);
    }
}
