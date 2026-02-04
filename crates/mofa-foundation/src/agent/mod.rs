//! Agent 基础构建块
//!
//! 包含 Agent 能力描述和组件 trait 定义

pub mod components;

// 重新导出核心类型 - 从 kernel 层统一导入
pub use mofa_kernel::agent::{
    AgentCapabilities, AgentRequirements, ReasoningStrategy,
};

// Re-export additional types needed by components
pub use mofa_kernel::agent::error::{AgentError, AgentResult};
pub use mofa_kernel::agent::context::AgentContext;
pub use mofa_kernel::agent::types::AgentInput;

// 重新导出组件
pub use components::{
    coordinator::{CoordinationPattern, Coordinator},
    memory::{Memory, MemoryItem, MemoryValue, Message, MessageRole, MemoryStats, InMemoryStorage, FileBasedStorage},
    reasoner::{Reasoner, ReasoningResult},
    tool::{Tool, ToolDescriptor, ToolInput, ToolMetadata, ToolResult},
};

/// Prelude 模块
pub mod prelude {
    pub use super::{
        AgentCapabilities, AgentRequirements, ReasoningStrategy,
    };
}
