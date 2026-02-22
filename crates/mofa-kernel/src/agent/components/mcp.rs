//! MCP (Model Context Protocol) 组件
//!
//! 定义 MCP 客户端接口，用于连接外部 MCP 服务器并使用其工具。
//!
//! # 概述
//!
//! MCP 是 Anthropic 提出的标准化协议，允许 AI Agent 与外部工具服务器通信。
//! 本模块定义了 MoFA 框架中的 MCP 客户端抽象。
//!
//! # 架构
//!
//! ```text
//! ┌─────────────────────────────────────────────┐
//! │              MoFA Agent                      │
//! │  ┌─────────────────────────────────────────┐│
//! │  │          ToolRegistry                    ││
//! │  │  ┌───────────┐  ┌───────────────────┐   ││
//! │  │  │ Built-in  │  │  McpToolAdapter   │   ││
//! │  │  │   Tools   │  │  (per MCP tool)   │   ││
//! │  │  └───────────┘  └────────┬──────────┘   ││
//! │  └──────────────────────────┼──────────────┘│
//! └─────────────────────────────┼───────────────┘
//!                               │
//!                    ┌──────────▼──────────┐
//!                    │    McpClient        │
//!                    │  (manages MCP       │
//!                    │   connections)      │
//!                    └──────────┬──────────┘
//!                               │
//!              ┌────────────────┼────────────────┐
//!              ▼                ▼                 ▼
//!     ┌──────────────┐ ┌──────────────┐ ┌──────────────┐
//!     │ MCP Server A │ │ MCP Server B │ │ MCP Server C │
//!     │  (stdio)     │ │   (HTTP)     │ │  (stdio)     │
//!     └──────────────┘ └──────────────┘ └──────────────┘
//! ```

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ============================================================================
// MCP Transport Configuration
// ============================================================================

/// MCP 服务器传输配置
///
/// 定义如何连接到 MCP 服务器
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum McpTransportConfig {
    /// 通过标准输入输出连接 (启动子进程)
    ///
    /// # 示例
    ///
    /// ```rust,ignore
    /// let config = McpTransportConfig::Stdio {
    ///     command: "npx".to_string(),
    ///     args: vec!["-y".to_string(), "@modelcontextprotocol/server-github".to_string()],
    ///     env: HashMap::from([("GITHUB_TOKEN".to_string(), "ghp_xxx".to_string())]),
    /// };
    /// ```
    Stdio {
        /// 可执行命令
        command: String,
        /// 命令参数
        args: Vec<String>,
        /// 环境变量
        env: HashMap<String, String>,
    },

    /// 通过 HTTP/SSE 连接
    ///
    /// # 示例
    ///
    /// ```rust,ignore
    /// let config = McpTransportConfig::Http {
    ///     url: "http://localhost:8080/mcp".to_string(),
    /// };
    /// ```
    Http {
        /// 服务器 URL
        url: String,
    },
}

// ============================================================================
// MCP Server Configuration
// ============================================================================

/// MCP 服务器配置
///
/// 定义单个 MCP 服务器的完整配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpServerConfig {
    /// 服务器名称 (用于标识)
    pub name: String,
    /// 传输配置
    pub transport: McpTransportConfig,
    /// 是否自动连接
    pub auto_connect: bool,
}

impl McpServerConfig {
    /// 创建 Stdio 类型的 MCP 服务器配置
    pub fn stdio(name: impl Into<String>, command: impl Into<String>, args: Vec<String>) -> Self {
        Self {
            name: name.into(),
            transport: McpTransportConfig::Stdio {
                command: command.into(),
                args,
                env: HashMap::new(),
            },
            auto_connect: true,
        }
    }

    /// 创建 HTTP 类型的 MCP 服务器配置
    pub fn http(name: impl Into<String>, url: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            transport: McpTransportConfig::Http { url: url.into() },
            auto_connect: true,
        }
    }

    /// 设置环境变量 (仅对 Stdio 有效)
    pub fn with_env(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        if let McpTransportConfig::Stdio { ref mut env, .. } = self.transport {
            env.insert(key.into(), value.into());
        }
        self
    }

    /// 设置是否自动连接
    pub fn with_auto_connect(mut self, auto_connect: bool) -> Self {
        self.auto_connect = auto_connect;
        self
    }
}

// ============================================================================
// MCP Tool Information
// ============================================================================

/// MCP 工具信息
///
/// 从 MCP 服务器发现的工具元信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpToolInfo {
    /// 工具名称
    pub name: String,
    /// 工具描述
    pub description: String,
    /// 输入参数 JSON Schema
    pub input_schema: serde_json::Value,
}

// ============================================================================
// MCP Server Information
// ============================================================================

/// MCP 服务器信息
///
/// MCP 服务器在初始化握手时返回的元信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpServerInfo {
    /// 服务器名称
    pub name: String,
    /// 服务器版本
    pub version: String,
    /// 服务器说明
    pub instructions: Option<String>,
}

// ============================================================================
// MCP Client Trait
// ============================================================================

/// MCP 客户端 Trait
///
/// 定义与 MCP 服务器通信的接口。
/// 具体实现在 `mofa-foundation` 层。
///
/// # 示例
///
/// ```rust,ignore
/// use mofa_kernel::agent::components::mcp::*;
///
/// let config = McpServerConfig::stdio(
///     "github",
///     "npx",
///     vec!["-y".into(), "@modelcontextprotocol/server-github".into()],
/// );
///
/// let mut client = MyMcpClient::new();
/// client.connect(config).await?;
///
/// let tools = client.list_tools("github").await?;
/// for tool in &tools {
///     println!("{}: {}", tool.name, tool.description);
/// }
///
/// let result = client.call_tool("github", "list_repos", json!({"owner": "mofa-org"})).await?;
/// ```
#[async_trait]
pub trait McpClient: Send + Sync {
    /// 连接到 MCP 服务器
    ///
    /// 使用给定配置建立与 MCP 服务器的连接。
    /// 连接建立后，可以通过 `server_name` 引用该服务器。
    async fn connect(&mut self, config: McpServerConfig) -> crate::agent::error::AgentResult<()>;

    /// 断开与 MCP 服务器的连接
    async fn disconnect(&mut self, server_name: &str) -> crate::agent::error::AgentResult<()>;

    /// 列出 MCP 服务器提供的工具
    async fn list_tools(
        &self,
        server_name: &str,
    ) -> crate::agent::error::AgentResult<Vec<McpToolInfo>>;

    /// 调用 MCP 服务器上的工具
    async fn call_tool(
        &self,
        server_name: &str,
        tool_name: &str,
        arguments: serde_json::Value,
    ) -> crate::agent::error::AgentResult<serde_json::Value>;

    /// 获取 MCP 服务器信息
    async fn server_info(
        &self,
        server_name: &str,
    ) -> crate::agent::error::AgentResult<McpServerInfo>;

    /// 获取所有已连接的服务器名称
    fn connected_servers(&self) -> Vec<String>;

    /// 检查服务器是否已连接
    fn is_connected(&self, server_name: &str) -> bool;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mcp_server_config_stdio() {
        let config = McpServerConfig::stdio("test-server", "node", vec!["server.js".to_string()])
            .with_env("API_KEY", "test-key")
            .with_auto_connect(false);

        assert_eq!(config.name, "test-server");
        assert!(!config.auto_connect);

        if let McpTransportConfig::Stdio { command, args, env } = &config.transport {
            assert_eq!(command, "node");
            assert_eq!(args, &["server.js"]);
            assert_eq!(env.get("API_KEY"), Some(&"test-key".to_string()));
        } else {
            panic!("Expected Stdio transport");
        }
    }

    #[test]
    fn test_mcp_server_config_http() {
        let config = McpServerConfig::http("api-server", "http://localhost:8080/mcp");

        assert_eq!(config.name, "api-server");
        assert!(config.auto_connect);

        if let McpTransportConfig::Http { url } = &config.transport {
            assert_eq!(url, "http://localhost:8080/mcp");
        } else {
            panic!("Expected Http transport");
        }
    }

    #[test]
    fn test_mcp_tool_info_serialization() {
        let tool = McpToolInfo {
            name: "list_repos".to_string(),
            description: "List GitHub repositories".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "owner": { "type": "string" }
                },
                "required": ["owner"]
            }),
        };

        let json = serde_json::to_string(&tool).unwrap();
        let deserialized: McpToolInfo = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.name, "list_repos");
    }

    #[test]
    fn test_mcp_server_info() {
        let info = McpServerInfo {
            name: "github".to_string(),
            version: "1.0.0".to_string(),
            instructions: Some("GitHub MCP Server".to_string()),
        };

        assert_eq!(info.name, "github");
        assert_eq!(info.version, "1.0.0");
        assert_eq!(info.instructions, Some("GitHub MCP Server".to_string()));
    }
}
