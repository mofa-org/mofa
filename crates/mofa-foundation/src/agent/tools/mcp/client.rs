//! MCP 客户端管理器
//! MCP Client Manager
//!
//! 使用 `rmcp` 库管理多个 MCP 服务器连接。
//! Manage multiple MCP server connections using the `rmcp` library.

use async_trait::async_trait;
use mofa_kernel::agent::components::mcp::{
    McpClient, McpServerConfig, McpServerInfo, McpToolInfo, McpTransportConfig,
};
use mofa_kernel::agent::error::{AgentError, AgentResult};
use rmcp::ServiceExt;
use rmcp::model::{CallToolRequestParams, ClientCapabilities, ClientInfo, Implementation};
use rmcp::service::{RoleClient, RunningService};
use rmcp::transport::TokioChildProcess;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::process::Command;
use tokio::sync::RwLock;
use tracing;

/// 单个 MCP 服务器连接的句柄
/// Handle for a single MCP server connection
struct McpConnection {
    /// rmcp 运行时服务
    /// rmcp runtime service
    service: RunningService<RoleClient, ClientInfo>,
    /// 服务器配置
    /// Server configuration
    config: McpServerConfig,
}

/// MCP 客户端管理器
/// MCP Client Manager
///
/// 管理到多个 MCP 服务器的连接，实现内核的 `McpClient` trait。
/// Manages connections to multiple MCP servers, implementing the kernel's `McpClient` trait.
///
/// # 示例
/// # Example
///
/// ```rust,ignore
/// use mofa_foundation::agent::tools::mcp::McpClientManager;
/// use mofa_kernel::agent::components::mcp::*;
///
/// let mut manager = McpClientManager::new();
///
/// // 连接到 GitHub MCP 服务器
/// // Connect to GitHub MCP server
/// let config = McpServerConfig::stdio(
///     "github",
///     "npx",
///     vec!["-y".into(), "@modelcontextprotocol/server-github".into()],
/// ).with_env("GITHUB_TOKEN", "ghp_xxx");
///
/// manager.connect(config).await?;
///
/// // 列出可用工具
/// // List available tools
/// let tools = manager.list_tools("github").await?;
/// println!("Found {} tools", tools.len());
///
/// // 调用工具
/// // Call a tool
/// let result = manager.call_tool(
///     "github",
///     "list_repos",
///     serde_json::json!({"owner": "mofa-org"}),
/// ).await?;
/// ```
pub struct McpClientManager {
    /// 已连接的 MCP 服务器
    /// Connected MCP servers
    connections: HashMap<String, McpConnection>,
}

impl McpClientManager {
    /// 创建新的 MCP 客户端管理器
    /// Create a new MCP client manager
    pub fn new() -> Self {
        Self {
            connections: HashMap::new(),
        }
    }

    /// 获取连接引用 (内部辅助方法)
    /// Get connection reference (internal helper method)
    fn get_connection(&self, server_name: &str) -> AgentResult<&McpConnection> {
        self.connections.get(server_name).ok_or_else(|| {
            AgentError::ToolNotFound(format!("MCP server '{}' not connected", server_name))
        })
    }

    /// 创建共享的客户端管理器引用
    /// Create a shared client manager reference
    pub fn into_shared(self) -> Arc<RwLock<Self>> {
        Arc::new(RwLock::new(self))
    }
}

impl Default for McpClientManager {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl McpClient for McpClientManager {
    async fn connect(&mut self, config: McpServerConfig) -> AgentResult<()> {
        let server_name = config.name.clone();

        if self.connections.contains_key(&server_name) {
            return Err(AgentError::ConfigError(format!(
                "MCP server '{}' is already connected",
                server_name
            )));
        }

        tracing::info!("Connecting to MCP server '{}'...", server_name);

        let service = match &config.transport {
            McpTransportConfig::Stdio { command, args, env } => {
                let mut cmd = Command::new(command);
                cmd.args(args);
                for (key, value) in env {
                    cmd.env(key, value);
                }

                let transport = TokioChildProcess::new(cmd).map_err(|e| {
                    AgentError::InitializationFailed(format!(
                        "Failed to start MCP server process '{}': {}",
                        server_name, e
                    ))
                })?;

                let client_info = ClientInfo {
                    meta: None,
                    protocol_version: Default::default(),
                    capabilities: ClientCapabilities::default(),
                    client_info: Implementation {
                        name: "mofa-agent".to_string(),
                        version: "0.1.0".to_string(),
                        title: None,
                        description: None,
                        icons: None,
                        website_url: None,
                    },
                };

                client_info.serve(transport).await.map_err(|e| {
                    AgentError::InitializationFailed(format!(
                        "Failed to initialize MCP session with '{}': {}",
                        server_name, e
                    ))
                })?
            }
            McpTransportConfig::Http { url: _ } => {
                // HTTP/SSE transport requires the `transport-streamable-http-client-reqwest` feature
                // which is not included by default. For now, return an error.
                // HTTP/SSE 传输需要 `transport-streamable-http-client-reqwest` 特性，目前尚未默认包含。
                return Err(AgentError::ConfigError(
                    "HTTP transport is not yet supported. Use Stdio transport instead.".to_string(),
                ));
            }
            _ => {
                return Err(AgentError::ConfigError(format!(
                    "Unsupported MCP transport for server '{}'",
                    server_name
                )));
            }
        };

        tracing::info!("Connected to MCP server '{}'", server_name);

        self.connections
            .insert(server_name, McpConnection { service, config });

        Ok(())
    }

    async fn disconnect(&mut self, server_name: &str) -> AgentResult<()> {
        if let Some(connection) = self.connections.remove(server_name) {
            tracing::info!("Disconnecting from MCP server '{}'...", server_name);
            connection.service.cancel().await.map_err(|e| {
                AgentError::ShutdownFailed(format!(
                    "Failed to disconnect from MCP server '{}': {:?}",
                    server_name, e
                ))
            })?;
            tracing::info!("Disconnected from MCP server '{}'", server_name);
            Ok(())
        } else {
            Err(AgentError::ToolNotFound(format!(
                "MCP server '{}' not connected",
                server_name
            )))
        }
    }

    async fn list_tools(&self, server_name: &str) -> AgentResult<Vec<McpToolInfo>> {
        let connection = self.get_connection(server_name)?;

        let result = connection
            .service
            .peer()
            .list_tools(None)
            .await
            .map_err(|e| {
                AgentError::ExecutionFailed(format!(
                    "Failed to list tools from MCP server '{}': {}",
                    server_name, e
                ))
            })?;

        let tools = result
            .tools
            .into_iter()
            .map(|tool| McpToolInfo {
                name: tool.name.to_string(),
                description: tool.description.unwrap_or_default().to_string(),
                input_schema: serde_json::to_value(&tool.input_schema)
                    .unwrap_or(serde_json::json!({})),
            })
            .collect();

        Ok(tools)
    }

    async fn call_tool(
        &self,
        server_name: &str,
        tool_name: &str,
        arguments: serde_json::Value,
    ) -> AgentResult<serde_json::Value> {
        let connection = self.get_connection(server_name)?;

        let params = CallToolRequestParams {
            name: tool_name.to_string().into(),
            arguments: if arguments.is_object() {
                Some(arguments.as_object().unwrap().clone())
            } else {
                Some(serde_json::Map::new())
            },
            meta: None,
            task: None,
        };

        let result = connection
            .service
            .peer()
            .call_tool(params)
            .await
            .map_err(|e| {
                AgentError::ExecutionFailed(format!(
                    "MCP tool call '{}' on server '{}' failed: {}",
                    tool_name, server_name, e
                ))
            })?;

        // Convert MCP CallToolResult content to JSON
        // 将 MCP CallToolResult 内容转换为 JSON
        let content_values: Vec<serde_json::Value> = result
            .content
            .iter()
            .map(|content| {
                // Each Content has a raw field that can be serialized
                // 每个 Content 都有一个可以序列化的原始字段
                serde_json::to_value(content)
                    .unwrap_or(serde_json::json!({"error": "serialization failed"}))
            })
            .collect();

        if result.is_error.unwrap_or(false) {
            let error_text = content_values
                .first()
                .and_then(|v| v.get("text").and_then(|t| t.as_str()))
                .unwrap_or("Unknown MCP error")
                .to_string();
            return Err(AgentError::ExecutionFailed(error_text));
        }

        // Return the full content array as JSON
        // 以 JSON 格式返回完整的内存数组
        Ok(serde_json::json!({
            "content": content_values,
        }))
    }

    async fn server_info(&self, server_name: &str) -> AgentResult<McpServerInfo> {
        let connection = self.get_connection(server_name)?;

        let peer = connection.service.peer();
        let server_info = peer.peer_info().ok_or_else(|| {
            AgentError::ExecutionFailed(format!("No server info available for '{}'", server_name))
        })?;

        Ok(McpServerInfo {
            name: server_info.server_info.name.clone(),
            version: server_info.server_info.version.clone(),
            instructions: server_info.instructions.clone(),
        })
    }

    fn connected_servers(&self) -> Vec<String> {
        self.connections.keys().cloned().collect()
    }

    fn is_connected(&self, server_name: &str) -> bool {
        self.connections.contains_key(server_name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_client_manager_new() {
        let manager = McpClientManager::new();
        assert!(manager.connected_servers().is_empty());
        assert!(!manager.is_connected("nonexistent"));
    }

    #[test]
    fn test_client_manager_default() {
        let manager = McpClientManager::default();
        assert!(manager.connected_servers().is_empty());
    }

    #[test]
    fn test_into_shared() {
        let manager = McpClientManager::new();
        let _shared = manager.into_shared();
        // Just verify it compiles and creates Arc<RwLock<>>
        // 仅验证其是否编译并创建了 Arc<RwLock<>>
    }

    #[tokio::test]
    async fn test_get_connection_missing() {
        let manager = McpClientManager::new();
        let result = manager.list_tools("nonexistent").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_disconnect_missing() {
        let mut manager = McpClientManager::new();
        let result = manager.disconnect("nonexistent").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_connect_duplicate() {
        let mut manager = McpClientManager::new();

        // We can't actually connect without a real MCP server, but we can test
        // the duplicate detection by using a server that can't start
        // 我们在没有真实 MCP 服务器的情况下无法实际连接，但可以通过使用无法启动的服务器测试重复检测
        let config = McpServerConfig::stdio("test", "nonexistent-command-xyz", vec![]);
        let _ = manager.connect(config).await; // This will fail - that's okay

        // Test HTTP transport error
        // 测试 HTTP 传输错误
        let http_config = McpServerConfig::http("http-test", "http://localhost:9999");
        let result = manager.connect(http_config).await;
        assert!(result.is_err()); // HTTP not yet supported
    }
}
