#![allow(
    dead_code,
    unused_imports,
    non_camel_case_types,
    ambiguous_glob_reexports
)]
// orchestrator module - Model Lifecycle & Allocation
pub mod orchestrator;

// model_pool module - Model lifecycle management with LRU cache
pub mod model_pool;

// routing module - Smart routing policy engine
pub mod routing;

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

// Validation middleware module
pub mod validation;

// Circuit breaker module - Retry and circuit breaker patterns
pub mod circuit_breaker;

// Re-export circuit breaker types
pub use circuit_breaker::{
    // Config
    CircuitBreakerConfig,
    AgentCircuitBreakerConfig,
    GlobalCircuitBreakerConfig,
    // State
    CircuitBreaker,
    AsyncCircuitBreaker,
    CircuitBreakerError,
    State,
    // Metrics
    CircuitBreakerMetrics,
    CircuitBreakerMetricsSnapshot,
    StateTransition,
    // Fallback
    FallbackStrategy,
    FallbackHandler,
    FallbackContext,
    FallbackError,
    execute_fallback,
    FallbackBuilder,
};

// Re-export validation types
pub use validation::{
    create_middleware, create_strict_middleware,
    EndpointValidationConfig,
    RateLimitConfig,
    RateLimitError,
    RateLimitKeyType,
    RateLimitResult,
    RateLimitStatus,
    RateLimiter,
    RequestContext,
    ResponseContext,
    SanitizerConfig,
    ValidationError,
    ValidationErrorCollection,
    ValidationMiddleware,
    ValidationMiddlewareConfig,
    ValidationOutcome,
    ValidationResult,
    ValidationRule,
    ValidationRuleType,
    SchemaValidator,
};

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

// Re-export secretary types for convenience
pub use secretary::{
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
    // Core types
    extract_json_block,
    parse_llm_json,
};
