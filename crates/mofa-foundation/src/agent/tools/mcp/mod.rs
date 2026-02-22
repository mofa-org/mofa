//! MCP 客户端实现
//!
//! 提供 MCP 客户端的具体实现，使用 `rmcp` 库连接 MCP 服务器。
//!
//! # 模块结构
//!
//! - `McpClientManager` — 管理多个 MCP 服务器连接
//! - `McpToolAdapter` — 将 MCP 工具包装为内核 `Tool` trait

mod client;
mod tool_adapter;

pub use client::McpClientManager;
pub use tool_adapter::McpToolAdapter;
