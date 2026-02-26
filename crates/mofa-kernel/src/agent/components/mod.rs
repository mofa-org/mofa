//! Agent 组件模块
//! Agent component module
//!
//! 定义 Agent 的可插拔组件接口
//! Defines the pluggable component interfaces for the Agent

pub mod context_compressor;
pub mod coordinator;
pub mod mcp;
pub mod memory;
pub mod reasoner;
pub mod tool;

pub use context_compressor::{CompressionStrategy, ContextCompressor};
pub use coordinator::{CoordinationPattern, Coordinator, DispatchResult, Task};
pub use mcp::{McpClient, McpServerConfig, McpServerInfo, McpToolInfo, McpTransportConfig};
pub use memory::{Memory, MemoryItem, MemoryStats, MemoryValue, Message, MessageRole};
pub use reasoner::{Decision, Reasoner, ReasoningResult, ThoughtStep};
pub use tool::{Tool, ToolDescriptor, ToolInput, ToolMetadata, ToolRegistry, ToolResult};
