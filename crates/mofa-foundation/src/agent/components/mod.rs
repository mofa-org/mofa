//! Agent 组件模块
//!
//! 定义 Agent 的可插拔组件接口

pub mod coordinator;
pub mod memory;
pub mod reasoner;
pub mod tool;

pub use coordinator::{CoordinationPattern, Coordinator, DispatchResult, Task};
pub use memory::{
    Memory, MemoryItem, MemoryValue, Message, MessageRole, MemoryStats,
    InMemoryStorage, FileBasedStorage,
};
pub use reasoner::{Decision, Reasoner, ReasoningResult, ThoughtStep};
pub use tool::{Tool, ToolDescriptor, ToolInput, ToolMetadata, ToolRegistry, ToolResult};
