//! Gateway external adapters for protocol implementations (MCP, etc.)

#[cfg(feature = "mcp")]
pub mod mcp;

#[cfg(feature = "mcp")]
pub use mcp::{McpAdapter, McpError};
