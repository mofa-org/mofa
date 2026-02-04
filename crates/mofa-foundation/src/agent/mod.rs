//! Agent 基础构建块
//!
//! 包含 Agent 能力描述和组件 trait 定义

pub mod components;
pub mod context;
pub mod session;
pub mod executor;
pub mod base;

// ========================================================================
// 从 Kernel 层重导出核心类型
// ========================================================================

pub use mofa_kernel::agent::{
    AgentCapabilities, AgentRequirements, ReasoningStrategy,
};

// Re-export additional types needed by components
pub use mofa_kernel::agent::error::{AgentError, AgentResult};
// Note: Using CoreAgentContext instead of deprecated AgentContext
pub use mofa_kernel::agent::context::CoreAgentContext;
pub use mofa_kernel::agent::types::AgentInput;

// 重新导出组件 (从 components 模块统一导入)
pub use components::{
    // Kernel traits 和类型 (通过 components 重导出)
    Coordinator, Reasoner, Tool,
    CoordinationPattern, DispatchResult, Task,
    Decision, ReasoningResult, ThoughtStep,
    LLMTool, ToolDescriptor, ToolInput, ToolMetadata, ToolRegistry, ToolResult,
    // Foundation 具体实现
    DirectReasoner, EchoTool, FileBasedStorage, InMemoryStorage,
    Memory, MemoryItem, MemoryStats, MemoryValue, Message, MessageRole,
    ParallelCoordinator, SequentialCoordinator, SimpleToolRegistry,
    // Foundation 扩展类型
    ToolCategory, ToolExt,
};

// Re-export context module
pub use context::{
    AgentIdentity, ContextExt, PromptContext, PromptContextBuilder, RichAgentContext,
};

// Re-export business types from rich context
pub use context::rich::{ComponentOutput, ExecutionMetrics};

// Re-export session module
pub use session::{
    JsonlSessionStorage, MemorySessionStorage, Session, SessionManager, SessionMessage,
    SessionStorage,
};

// Re-export executor module
pub use executor::{
    AgentExecutor, AgentExecutorConfig,
};

// Re-export LLM types from kernel
pub use mofa_kernel::agent::types::{
    ChatCompletionRequest, ChatCompletionResponse, ChatMessage, LLMProvider,
    ToolCall, ToolDefinition, TokenUsage,
};

// Re-export BaseAgent from base module
pub use base::BaseAgent;

// Note: Secretary abstract traits are in mofa_kernel::agent::secretary
// Foundation layer provides concrete implementations
// Use mofa_kernel::agent::secretary for traits, or mofa_foundation::secretary for implementations

/// Prelude 模块
pub mod prelude {
    pub use super::{
        AgentCapabilities, AgentRequirements, ReasoningStrategy,
    };
}
