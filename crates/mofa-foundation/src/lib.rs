#![allow(
    dead_code,
    unused_imports,
    non_camel_case_types,
    ambiguous_glob_reexports
)]
// orchestrator module - Model Lifecycle & Allocation
pub mod orchestrator;

// hardware discovery module
pub mod hardware;

// prompt module
pub mod prompt;

// react module
pub mod react;

// messaging module
pub mod messaging;

// persistence module
pub mod persistence;

// llm module
pub mod llm;

// workflow module
pub mod workflow;

// coordination module
pub mod coordination;

// config module
pub mod config;

// secretary module - 秘书Agent模式
pub mod secretary;

// collaboration module - 自适应协作协议
pub mod agent;
pub mod collaboration;

// RAG module - vector store and document chunking
pub mod rag;

// Re-export config types
pub use config::{AgentInfo, AgentYamlConfig, LLMYamlConfig, RuntimeConfig, ToolConfig};

// Re-export messaging types
pub use messaging::{
    InboundMessage, MessageBus, OutboundMessage, SimpleInboundMessage, SimpleOutboundMessage,
};

// Re-export prompt types
pub use prompt::{
    ConversationBuilder, GlobalPromptRegistry, PromptBuilder, PromptComposition, PromptError,
    PromptRegistry, PromptResult, PromptTemplate, PromptVariable, VariableType,
};

// Re-export orchestrator types (GSoC 2026 Edge Model Orchestrator)
pub use orchestrator::{
    DegradationLevel, ModelOrchestrator, ModelProvider, ModelProviderConfig, ModelType,
    OrchestratorError, OrchestratorResult, PoolStatistics,
};

// Re-export Linux implementation and pipeline when available
#[cfg(target_os = "linux")]
pub use orchestrator::{
    InferencePipeline, LinuxCandleProvider, ModelPool, PipelineBuilder, PipelineOutput,
    PipelineStage,
};

// Re-export secretary types for convenience
pub use secretary::{
    // Core types
    extract_json_block,
    parse_llm_json,
    Artifact,
    ChannelConnection,
    ChatMessage,
    // Connection
    CriticalDecision,
    DecisionOption,
    // LLM
    DecisionType,
    DefaultInput,
    DefaultOutput,
    DefaultSecretaryBehavior,
    // Default implementation
    DefaultSecretaryBuilder,
    ExecutionResult,
    HumanResponse,
    // LLM integration
    LLMProvider,
    ProjectRequirement,
    QueryType,
    // Command types
    Report,
    ReportType,
    Resource,
    // Task types
    SecretaryBehavior,
    SecretaryCommand,
    SecretaryContext,
    SecretaryCore,
    SecretaryEvent,
    SecretaryHandle,
    SecretaryMessage,
    Subtask,
    TaskExecutionStatus,
    TodoItem,
    TodoPriority,
    TodoStatus,
    UserConnection,
    WorkPhase,
};
