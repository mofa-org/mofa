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
pub mod collaboration;
pub mod agent;

// Re-export config types
pub use config::{AgentInfo, AgentYamlConfig, LLMYamlConfig, RuntimeConfig, ToolConfig};

// Re-export messaging types
pub use messaging::{MessageBus, InboundMessage, OutboundMessage, SimpleInboundMessage, SimpleOutboundMessage};

// Re-export prompt types
pub use prompt::{
    ConversationBuilder, GlobalPromptRegistry, PromptBuilder, PromptComposition, PromptError,
    PromptRegistry, PromptResult, PromptTemplate, PromptVariable, VariableType,
};

// Re-export secretary types for convenience
pub use secretary::{
    // Core types
    extract_json_block, parse_llm_json, Artifact, ChannelConnection, ChatMessage,
    // Connection
    CriticalDecision, DecisionOption,
    // LLM
    DecisionType, DefaultInput, DefaultOutput, DefaultSecretaryBehavior,
    // Default implementation
    DefaultSecretaryBuilder, ExecutionResult, ExecutorCapability, HumanResponse,
    // Legacy aliases
    LLMProvider, ProjectRequirement, QueryType,
    // Command types
    Report, ReportType, Resource,
    // Task types
    SecretaryAgent, SecretaryAgentBuilder, SecretaryBehavior, SecretaryCommand, SecretaryContext, SecretaryCore,
    SecretaryEvent, SecretaryHandle, SecretaryMessage, Subtask, TaskExecutionStatus, TodoItem,
    TodoPriority, TodoStatus, UserConnection, UserInput, WorkPhase,
};
