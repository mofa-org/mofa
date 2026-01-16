//! Agent 基础构建块
//!
//! 包含 Agent 能力描述和组件 trait 定义

pub mod components;

// 重新导出核心类型 - 从 kernel 层统一导入
pub use mofa_kernel::agent::{
    AgentCapabilities, AgentRequirements, ReasoningStrategy,
};

// 重新导出组件
pub use components::{
    coordinator::{CoordinationPattern, Coordinator},
    memory::{Memory, MemoryItem, MemoryValue},
    reasoner::{Reasoner, ReasoningResult},
    tool::{Tool, ToolDescriptor, ToolInput, ToolMetadata, ToolResult},
};

/// Prelude 模块
pub mod prelude {
    pub use super::{
        AgentCapabilities, AgentRequirements, ReasoningStrategy,
    };
}
