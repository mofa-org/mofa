//! Agent 组件模块
//! Agent component module
//!
//! 定义 Agent 的可插拔组件接口
//! Defines pluggable component interfaces for the Agent
//!
//! ## 架构说明
//! ## Architecture Description
//!
//! 本模块从 kernel 层导入核心 trait 接口，并提供具体实现。
//! This module imports core trait interfaces from the kernel layer and provides concrete implementations.
//! - Kernel 层定义 trait 接口（Reasoner, Coordinator, Tool, Memory 等）
//! - Kernel layer defines trait interfaces (Reasoner, Coordinator, Tool, Memory, etc.)
//! - Foundation 层提供具体实现（DirectReasoner, SequentialCoordinator, SimpleToolRegistry 等）
//! - Foundation layer provides concrete implementations (DirectReasoner, SequentialCoordinator, SimpleToolRegistry, etc.)

pub mod context_compressor;
pub mod coordinator;
pub mod memory;
pub mod reasoner;
pub mod tool;

// Note: tool_registry was removed - SimpleToolRegistry and EchoTool are now in tool.rs

// ============================================================================
// 重新导出 Kernel 层类型 (直接导入以确保可见性)
// Re-export Kernel layer types (direct import to ensure visibility)
// ============================================================================

// Context compressor - Kernel trait and types
pub use mofa_kernel::agent::components::context_compressor::{
    CompressionStrategy, ContextCompressor,
};

// Context compressor - Foundation implementations
pub use context_compressor::{SlidingWindowCompressor, SummarizingCompressor, TokenCounter};

// Coordinator - Kernel trait 和类型
// Coordinator - Kernel trait and types
pub use mofa_kernel::agent::components::coordinator::{
    AggregationStrategy, CoordinationPattern, Coordinator, DispatchResult, DispatchStatus, Task,
    TaskPriority, TaskType, aggregate_outputs,
};

// Reasoner - Kernel trait 和类型
// Reasoner - Kernel trait and types
pub use mofa_kernel::agent::components::reasoner::{
    Decision, Reasoner, ReasoningResult, ThoughtStep, ThoughtStepType, ToolCall,
};

// Tool - Kernel trait 和类型
// Tool - Kernel trait and types
pub use mofa_kernel::agent::components::tool::{
    LLMTool, Tool, ToolDescriptor, ToolInput, ToolMetadata, ToolRegistry, ToolResult,
};

// ============================================================================
// Foundation 层具体实现
// Foundation layer concrete implementations
// ============================================================================

// Coordinator 实现
// Coordinator implementations
pub use coordinator::{ParallelCoordinator, SequentialCoordinator};

// Memory 实现 (Foundation 独有)
// Memory implementations (Exclusive to Foundation)
pub use memory::{
    FileBasedStorage, InMemoryStorage, Memory, MemoryItem, MemoryStats, MemoryValue, Message,
    MessageRole,
};

// Reasoner 实现
// Reasoner implementations
pub use reasoner::DirectReasoner;

// Tool 扩展和实现
// Tool extensions and implementations
pub use tool::{
    EchoTool, SimpleTool, SimpleToolAdapter, SimpleToolRegistry, ToolCategory, ToolExt, as_tool,
};
