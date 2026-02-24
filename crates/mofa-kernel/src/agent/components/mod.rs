//! Agent 组件模块
//! Agent component module
//!
//! 定义 Agent 的可插拔组件接口
//! Defines the pluggable component interfaces for the Agent

pub mod coordinator;
pub mod mcp;
pub mod memory;
pub mod reasoner;
pub mod tool;

pub use coordinator::{CoordinationPattern, Coordinator, DispatchResult, Task};
pub use mcp::{McpClient, McpServerConfig, McpServerInfo, McpToolInfo, McpTransportConfig};
pub use memory::{Embedder, Memory, MemoryItem, MemoryStats, MemoryValue, Message, MessageRole};
pub use reasoner::{Decision, Reasoner, ReasoningResult, ThoughtStep};
pub use tool::{Tool, ToolDescriptor, ToolInput, ToolMetadata, ToolRegistry, ToolResult};
