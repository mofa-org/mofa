//! Gateway external adapters for protocol implementations (A2A, MCP, etc.)

pub mod a2a;
#[cfg(feature = "mcp")]
pub mod mcp;

pub use a2a::{A2aAdapter, AgentCard, A2aTask, A2aTaskStatus};
#[cfg(feature = "mcp")]
pub use mcp::{McpAdapter, McpError};
