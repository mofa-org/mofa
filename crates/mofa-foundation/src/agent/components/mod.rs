//! Agent 组件模块
//!
//! 定义 Agent 的可插拔组件接口
//!
//! ## 架构说明
//!
//! 本模块从 kernel 层导入核心 trait 接口，并提供具体实现。
//! - Kernel 层定义 trait 接口（Reasoner, Coordinator, Tool, Memory 等）
//! - Foundation 层提供具体实现（DirectReasoner, SequentialCoordinator, SimpleToolRegistry 等）

pub mod coordinator;
pub mod memory;
pub mod reasoner;
pub mod tool;

// Note: tool_registry was removed - SimpleToolRegistry and EchoTool are now in tool.rs

// ============================================================================
// 重新导出 Kernel 层类型 (直接导入以确保可见性)
// ============================================================================

// Coordinator - Kernel trait 和类型
pub use mofa_kernel::agent::components::coordinator::{
    AggregationStrategy, CoordinationPattern, Coordinator, DispatchResult, DispatchStatus, Task,
    TaskPriority, TaskType, aggregate_outputs,
};

// Reasoner - Kernel trait 和类型
pub use mofa_kernel::agent::components::reasoner::{
    Decision, Reasoner, ReasoningResult, ThoughtStep, ThoughtStepType, ToolCall,
};

// Tool - Kernel trait 和类型
pub use mofa_kernel::agent::components::tool::{
    LLMTool, Tool, ToolDescriptor, ToolInput, ToolMetadata, ToolRegistry, ToolResult,
};

// ============================================================================
// Foundation 层具体实现
// ============================================================================

// Coordinator 实现
pub use coordinator::{ParallelCoordinator, SequentialCoordinator};

// Memory 实现 (Foundation 独有)
pub use memory::{
    FileBasedStorage, InMemoryStorage, Memory, MemoryItem, MemoryStats, MemoryValue, Message,
    MessageRole,
};

// Reasoner 实现
pub use reasoner::DirectReasoner;

// Tool 扩展和实现
pub use tool::{
    EchoTool, SimpleTool, SimpleToolAdapter, SimpleToolRegistry, ToolCategory, ToolExt, as_tool,
};
