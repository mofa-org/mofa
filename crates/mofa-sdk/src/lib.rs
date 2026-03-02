//! MoFA API - Standard SDK for MoFA framework
//!
//! This crate provides a standardized API for the MoFA (Model-based Framework for Agents) framework.
//!
//! # Architecture Layers
//!
//! The SDK is organized into clear layers following microkernel architecture principles:
//!
//! ```text
//! ┌─────────────────────────────────────────┐
//! │            User Code                    │
//! └─────────────────┬───────────────────────┘
//!                   ↓
//! ┌─────────────────────────────────────────┐
//! │     SDK (Standard API Surface)          │
//! │  - kernel: Core abstractions            │
//! │  - runtime: Lifecycle management        │
//! │  - foundation: Business functionality   │
//! └─────────────────┬───────────────────────┘
//! ```
//!
//! # Features
//!
//! - `dora` - Enable dora-rs runtime support for distributed dataflow
//!
//! For FFI bindings (Python, Kotlin, Swift, Java), use the `mofa-ffi` crate.
//!
//! # Quick Start
//!
//! ```toml
//! mofa-sdk = "0.1"
//! ```
//!
//! ```rust,ignore
//! use mofa_sdk::kernel::{AgentInput, MoFAAgent};
//! use mofa_sdk::runtime::run_agents;
//!
//! struct MyAgent;
//!
//! #[async_trait::async_trait]
//! impl MoFAAgent for MyAgent {
//!     // implementation...
//! }
//!
//! #[tokio::main]
//! async fn main() -> GlobalResult<()> {
//!     let outputs = run_agents(MyAgent, vec![AgentInput::text("Hello")]).await?;
//!     println!("{}", outputs[0].to_text());
//!     Ok(())
//! }
//! ```

// =============================================================================
// Kernel Layer - Core Abstractions
// =============================================================================

/// Core agent abstractions and extensions
///
/// This module provides the minimal core interfaces that all agents implement.
/// Following microkernel principles, the core is kept minimal with optional
/// extensions for additional capabilities.
///
/// # Core Trait
///
/// - `MoFAAgent`: The core agent interface (id, name, capabilities, execute, etc.)
///
/// # Extension Traits
///
/// - `AgentLifecycle`: pause, resume, interrupt
/// - `AgentMessaging`: handle_message, handle_event
/// - `AgentPluginSupport`: plugin management
///
/// # Example
///
/// ```rust,ignore
/// use mofa_sdk::kernel::MoFAAgent;
///
/// #[async_trait::async_trait]
/// impl MoFAAgent for MyAgent {
///     fn id(&self) -> &str { "my-agent" }
///     fn name(&self) -> &str { "My Agent" }
///     // ... other methods
/// }
/// ```
pub mod kernel {
    // Core abstractions and infrastructure from `mofa-kernel`.
    //
    // This module is a normalized, comprehensive facade over `mofa-kernel` with
    // structured submodules and curated top-level re-exports.

    // ---------------------------------------------------------------------
    // Structured submodules (full coverage)
    // ---------------------------------------------------------------------
    pub mod agent {
        pub use mofa_kernel::agent::*;
    }
    pub mod message {
        pub use mofa_kernel::message::*;
    }
    pub mod bus {
        pub use mofa_kernel::bus::*;
    }
    pub mod plugin {
        pub use mofa_kernel::plugin::*;
    }
    pub mod config {
        pub use mofa_kernel::config::*;
    }
    pub mod core {
        pub use mofa_kernel::core::*;
    }
    pub mod storage {
        pub use mofa_kernel::storage::*;
    }

    // ---------------------------------------------------------------------
    // Curated, commonly-used exports
    // ---------------------------------------------------------------------
    pub use mofa_kernel::agent::{
        AgentCapabilities, AgentCapabilitiesBuilder, AgentContext, AgentError, AgentFactory,
        AgentInput, AgentLifecycle, AgentMessage as CoreAgentMessage, AgentMessaging,
        AgentMetadata, AgentOutput, AgentPluginSupport, AgentRequirements,
        AgentRequirementsBuilder, AgentResult, AgentState, AgentStats, ChatCompletionRequest,
        ChatCompletionResponse, ChatMessage, ContextConfig, CoordinationPattern, Coordinator,
        DynAgent, ErrorCategory, ErrorContext, EventBuilder, EventBus, GlobalError, GlobalEvent,
        GlobalMessage, GlobalResult, HealthStatus, InputType, InterruptResult, LLMProvider, Memory,
        MemoryItem, MemoryStats, MemoryValue, Message, MessageContent, MessageMetadata,
        MessageRole, MoFAAgent, OutputContent, OutputType, Reasoner, ReasoningResult,
        ReasoningStep, ReasoningStepType, ReasoningStrategy, TokenUsage, Tool, ToolCall,
        ToolDefinition, ToolDescriptor, ToolInput, ToolMetadata, ToolResult, ToolUsage,
        execution_events, lifecycle, message_events, plugin_events, state_events,
    };

    // Core AgentConfig (runtime-level, lightweight)
    pub use mofa_kernel::core::AgentConfig;

    // Schema/config types for agent definitions
    pub use mofa_kernel::agent::config::{
        AgentConfig as AgentSchemaConfig, AgentType, ConfigFormat, ConfigLoader,
    };

    // Message-level events and task primitives (stream + scheduling included)
    pub use mofa_kernel::message::{
        AgentEvent, AgentMessage, SchedulingStatus, StreamControlCommand, StreamType, TaskPriority,
        TaskRequest, TaskStatus,
    };

    // Bus
    pub use mofa_kernel::bus::AgentBus;

    // Plugin primitives
    pub use mofa_kernel::plugin::{
        AgentPlugin, HotReloadConfig, PluginContext, PluginEvent, PluginMetadata, PluginResult,
        PluginState, PluginType, ReloadEvent, ReloadStrategy,
    };

    // Storage trait
    pub use mofa_kernel::Storage;
}

// =============================================================================
// Runtime Layer - Lifecycle and Execution
// =============================================================================

/// Agent lifecycle and execution management
///
/// This module provides runtime infrastructure for managing agent execution.
///
/// # Main Components
///
/// - `AgentBuilder`: Builder pattern for constructing agents
/// - `SimpleRuntime`: Multi-agent coordination (non-dora)
/// - `AgentRuntime`: Dora-rs integration (with `dora` feature)
///
/// # Example
///
/// ```rust,ignore
/// use mofa_sdk::runtime::{AgentBuilder, SimpleRuntime};
///
/// let runtime = SimpleRuntime::new();
/// runtime.register_agent(metadata, config, "worker").await?;
/// ```
pub mod runtime {
    // Agent builder
    pub use mofa_runtime::AgentBuilder;

    // Simple runtime (non-dora mode only)
    #[cfg(not(feature = "dora"))]
    pub use mofa_runtime::SimpleRuntime;

    // Agent registry (runtime implementation)
    pub use mofa_runtime::agent::{AgentFactory, AgentRegistry, RegistryStats};

    // Agent runner (single-execution utilities)
    pub use mofa_runtime::runner::{
        AgentRunner, AgentRunnerBuilder, RunnerState, RunnerStats, run_agents,
    };

    pub use mofa_runtime::config::FrameworkConfig;

    // Dora runtime (only available with dora feature)
    #[cfg(feature = "dora")]
    pub use mofa_runtime::{AgentRuntime, MoFARuntime};
}

// =============================================================================
// Agent Layer - Foundation Agent Building Blocks
// =============================================================================

/// Agent building blocks and concrete implementations (foundation layer)
pub mod agent {
    pub use mofa_foundation::agent::*;
}

// =============================================================================
// Prompt Layer - Prompt Composition & Management
// =============================================================================

/// Prompt templates, registries, and composition utilities
pub mod prompt {
    pub use mofa_foundation::prompt::*;
}

// =============================================================================
// Coordination Layer - Task Coordination
// =============================================================================

/// Coordination strategies and schedulers (foundation layer)
pub mod coordination {
    pub use mofa_foundation::coordination::*;
}

// =============================================================================
// Config Layer - Global Configuration
// =============================================================================

/// Global configuration facade (kernel + runtime + foundation)
pub mod config {
    /// Kernel config helpers and loaders
    pub mod kernel {
        pub use mofa_kernel::agent::config::*;
        pub use mofa_kernel::config::*;
        pub use mofa_kernel::core::AgentConfig as CoreAgentConfig;
    }

    /// Runtime config
    pub mod runtime {
        pub use mofa_runtime::config::*;
    }

    /// Foundation YAML config
    pub mod foundation {
        pub use mofa_foundation::config::*;
    }

    // Curated top-level re-exports
    pub use mofa_foundation::config::{
        AgentInfo, AgentYamlConfig, LLMYamlConfig, RuntimeConfig as YamlRuntimeConfig, ToolConfig,
    };
    pub use mofa_runtime::config::FrameworkConfig;
}

// =============================================================================
// Foundation Layer - Business Functionality
// =============================================================================

/// Business functionality and concrete implementations
///
/// This module provides production-ready agent implementations and business logic.
///
/// # Modules
///
/// - `llm`: LLM integration (OpenAI, etc.)
/// - `secretary`: Secretary agent pattern
/// - `react`: ReAct (Reasoning + Acting) framework
/// - `collaboration`: Multi-agent collaboration protocols
/// - `persistence`: Database persistence
pub mod foundation {
    pub use super::agent;
    pub use super::collaboration;
    pub use super::config;
    pub use super::coordination;
    pub use super::llm;
    pub use super::messaging;
    pub use super::persistence;
    pub use super::prompt;
    pub use super::react;
    pub use super::secretary;
    pub use super::workflow;
}

// =============================================================================
// Plugins (explicit module)
// =============================================================================

pub mod plugins {
    pub use mofa_plugins::{
        AgentPlugin,
        AudioPlaybackConfig,
        LLMPlugin,
        LLMPluginConfig,
        MemoryPlugin,
        MemoryStorage,
        MockTTSEngine,
        // Kernel plugin primitives
        PluginConfig,
        PluginContext,
        PluginError,
        PluginEvent,
        PluginManager,
        PluginMetadata,
        PluginResult,
        PluginState,
        PluginType,
        RhaiPlugin,
        RhaiPluginConfig,
        RhaiPluginState,
        StoragePlugin,
        TTSCommand,
        TTSEngine,
        // TTS plugin types
        TTSPlugin,
        TTSPluginConfig,
        TextToSpeechTool,
        ToolCall,
        ToolDefinition,
        ToolExecutor,
        ToolPlugin,
        ToolPluginAdapter,
        ToolResult,
        VoiceInfo,
        adapt_tool,
        // TTS audio playback function
        play_audio,
        play_audio_async,
        // Runtime plugin creation helpers
        rhai_runtime,
        tool,
        tools,
        wasm_runtime,
    };

    pub use mofa_kernel::PluginPriority;

    // Re-export KokoroTTSWrapper when kokoro feature is enabled
    #[cfg(feature = "kokoro")]
    pub use mofa_plugins::KokoroTTS;

    // Hot reload utilities
    pub mod hot_reload {
        pub use mofa_plugins::hot_reload::*;
    }
}

// =============================================================================
// Workflow (explicit module)
// =============================================================================

pub mod workflow {
    //! Workflow orchestration module with LangGraph-inspired StateGraph API
    //!
    //! # StateGraph API (Recommended)
    //!
    //! The new StateGraph API provides a more intuitive way to build workflows:
    //!
    //! ```rust,ignore
    //! use mofa_sdk::workflow::{StateGraphImpl, AppendReducer, OverwriteReducer, StateGraph, START, END};
    //!
    //! let graph = StateGraphImpl::<MyState>::new("my_workflow")
    //!     .add_reducer("messages", Box::new(AppendReducer))
    //!     .add_node("process", Box::new(ProcessNode))
    //!     .add_edge(START, "process")
    //!     .add_edge("process", END)
    //!     .compile()?;
    //!
    //! let result = graph.invoke(initial_state, None).await?;
    //! ```
    //!
    //! # Legacy Workflow API
    //!
    //! The original WorkflowGraph API is still available for backward compatibility.

    // Re-export kernel workflow types
    pub use mofa_kernel::workflow::{
        Command, CompiledGraph, ControlFlow, END, EdgeTarget, GraphConfig, GraphState, JsonState,
        NodeFunc, Reducer, ReducerType, RemainingSteps, RuntimeContext, START, SendCommand,
        StateSchema, StateUpdate, StepResult, StreamEvent,
    };

    // Re-export kernel StateGraph trait
    pub use mofa_kernel::workflow::StateGraph;

    // Foundation layer implementations
    pub use mofa_foundation::workflow::{
        // Reducers
        AppendReducer,
        // StateGraph implementation
        CompiledGraphImpl,
        CustomReducer,
        ExtendReducer,
        FirstReducer,
        LastNReducer,
        LastReducer,
        MergeReducer,
        OverwriteReducer,
        StateGraphImpl,
        create_reducer,
    };

    // Legacy workflow API
    pub use mofa_foundation::workflow::{
        ExecutionEvent, ExecutorConfig, WorkflowBuilder, WorkflowExecutor, WorkflowGraph,
        WorkflowNode, WorkflowValue,
    };

    // DSL support
    pub use mofa_foundation::workflow::dsl::{
        AgentRef, DslError, DslResult, EdgeDefinition, LlmAgentConfig, LoopConditionDef,
        NodeConfigDef, NodeDefinition, RetryPolicy, TaskExecutorDef, TimeoutConfig, TransformDef,
        WorkflowConfig, WorkflowDefinition, WorkflowDslParser, WorkflowMetadata,
    };
}

// =============================================================================
// Prelude - Commonly Used Imports
// =============================================================================

/// Commonly used types for quick start
pub mod prelude {
    pub use crate::kernel::{
        AgentCapabilities, AgentCapabilitiesBuilder, AgentContext, AgentError, AgentInput,
        AgentMetadata, AgentOutput, AgentResult, AgentState, MoFAAgent,
    };
    #[cfg(not(feature = "dora"))]
    pub use crate::runtime::SimpleRuntime;
    pub use crate::runtime::{AgentBuilder, AgentRunner, run_agents};
    pub use async_trait::async_trait;
}

// Re-export dashboard module (only available with monitoring feature)
#[cfg(feature = "monitoring")]
pub mod dashboard {
    pub use mofa_monitoring::*;
}

// Rhai scripting helpers (explicit module)
pub mod rhai {
    pub use mofa_extra::rhai::*;
}

mod llm_tools;

// Re-export LLM module from mofa-foundation (always available)
pub mod llm {
    //! LLM (Large Language Model) integration module
    //!
    //! Provides LLM interaction capabilities for agents.
    //!
    //! # Quick Start
    //!
    //! ```rust,ignore
    //! use mofa_sdk::llm::{LLMProvider, LLMClient, ChatMessage, ChatCompletionRequest};
    //!
    //! // Implement your LLM provider
    //! struct MyProvider { /* ... */ }
    //!
    //! #[async_trait::async_trait]
    //! impl LLMProvider for MyProvider {
    //!     fn name(&self) -> &str { "my-llm" }
    //!     async fn chat(&self, request: ChatCompletionRequest) -> LLMResult<ChatCompletionResponse> {
    //!         // Your implementation
    //!     }
    //! }
    //!
    //! // Use the client
    //! let client = LLMClient::new(Arc::new(MyProvider::new()));
    //! let answer = client.ask("What is Rust?").await?;
    //! ```

    pub use crate::llm_tools::ToolPluginExecutor;
    pub use mofa_foundation::llm::anthropic::{AnthropicConfig, AnthropicProvider};
    pub use mofa_foundation::llm::google::{GeminiConfig, GeminiProvider};
    pub use mofa_foundation::llm::ollama::{OllamaConfig, OllamaProvider};
    pub use mofa_foundation::llm::openai::{OpenAIConfig, OpenAIProvider};
    pub use mofa_foundation::llm::*;

    /// 从环境变量创建 OpenAI 提供器
    /// Create OpenAI provider from environment variables
    ///
    /// 自动读取以下环境变量:
    /// Automatically reads the following environment variables:
    /// - OPENAI_API_KEY: API 密钥
    /// - OPENAI_API_KEY: API Key
    /// - OPENAI_BASE_URL: 可选的 API 基础 URL
    /// - OPENAI_BASE_URL: Optional API Base URL
    /// - OPENAI_MODEL: 可选的默认模型
    /// - OPENAI_MODEL: Optional default model
    ///
    /// # 示例
    /// # Example
    ///
    /// ```rust,ignore
    /// use mofa_sdk::llm::openai_from_env;
    ///
    /// let provider = openai_from_env().unwrap();
    /// ```
    pub fn openai_from_env() -> Result<OpenAIProvider, crate::llm::LLMError> {
        let api_key = std::env::var("OPENAI_API_KEY").map_err(|_| {
            crate::llm::LLMError::ConfigError(
                "OpenAI API key not found in environment variable OPENAI_API_KEY".to_string(),
            )
        })?;

        let mut config = OpenAIConfig::new(api_key);

        if let Ok(base_url) = std::env::var("OPENAI_BASE_URL") {
            config = config.with_base_url(&base_url);
        }

        if let Ok(model) = std::env::var("OPENAI_MODEL") {
            config = config.with_model(&model);
        }

        Ok(OpenAIProvider::with_config(config))
    }

    /// Create an Ollama provider from environment variables (no API key required).
    ///
    /// Reads:
    /// - `OLLAMA_BASE_URL`: base URL without `/v1` suffix, e.g. `http://localhost:11434` (optional)
    /// - `OLLAMA_MODEL`: model name, e.g. `llama3` (optional)
    pub fn ollama_from_env() -> Result<OllamaProvider, crate::llm::LLMError> {
        Ok(crate::llm::OllamaProvider::from_env())
    }
}

/// 从环境变量创建 Anthropic 提供器
/// Create Anthropic provider from environment variables
///
/// 读取环境变量:
/// Reads environment variables:
/// - ANTHROPIC_API_KEY (必需)
/// - ANTHROPIC_API_KEY (Required)
/// - ANTHROPIC_BASE_URL (可选)
/// - ANTHROPIC_BASE_URL (Optional)
/// - ANTHROPIC_MODEL (可选)
/// - ANTHROPIC_MODEL (Optional)
pub fn anthropic_from_env() -> Result<crate::llm::AnthropicProvider, crate::llm::LLMError> {
    let api_key = std::env::var("ANTHROPIC_API_KEY").map_err(|_| {
        crate::llm::LLMError::ConfigError(
            "Anthropic API key not found in ANTHROPIC_API_KEY".to_string(),
        )
    })?;

    let mut cfg = crate::llm::AnthropicConfig::new(api_key);
    if let Ok(base_url) = std::env::var("ANTHROPIC_BASE_URL") {
        cfg = cfg.with_base_url(base_url);
    }
    if let Ok(model) = std::env::var("ANTHROPIC_MODEL") {
        cfg = cfg.with_model(model);
    }

    Ok(crate::llm::AnthropicProvider::with_config(cfg))
}

/// 从环境变量创建 Google Gemini 提供器
/// Create Google Gemini provider from environment variables
///
/// 读取环境变量:
/// Reads environment variables:
/// - GEMINI_API_KEY (必需)
/// - GEMINI_API_KEY (Required)
/// - GEMINI_BASE_URL (可选)
/// - GEMINI_BASE_URL (Optional)
/// - GEMINI_MODEL (可选)
/// - GEMINI_MODEL (Optional)
pub fn gemini_from_env() -> Result<crate::llm::GeminiProvider, crate::llm::LLMError> {
    let api_key = std::env::var("GEMINI_API_KEY").map_err(|_| {
        crate::llm::LLMError::ConfigError("Gemini API key not found in GEMINI_API_KEY".to_string())
    })?;

    let mut cfg = crate::llm::GeminiConfig::new(api_key);
    if let Ok(base_url) = std::env::var("GEMINI_BASE_URL") {
        cfg = cfg.with_base_url(base_url);
    }
    if let Ok(model) = std::env::var("GEMINI_MODEL") {
        cfg = cfg.with_model(model);
    }

    Ok(crate::llm::GeminiProvider::with_config(cfg))
}

// Re-export Secretary module from mofa-foundation (always available)
pub mod secretary {
    //! 秘书Agent模式 - 基于事件循环的智能助手
    //! Secretary Agent Mode - Intelligent assistant based on event loop
    //!
    //! 秘书Agent是一个面向用户的智能助手，通过与LLM交互完成个人助理工作。
    //! Secretary Agent is a user-facing smart assistant completing personal aid tasks via LLM interaction.
    //! 设计为与长连接配合使用，实现持续的交互式服务。
    //! Designed to work with long-lived connections for continuous interactive services.
    //!
    //! ## 工作循环（5阶段事件循环）
    //! ## Work Cycle (5-phase event loop)
    //!
    //! 1. **接收想法** → 记录并生成TODO
    //! 1. **Receive Ideas** → Log and generate TODOs
    //! 2. **澄清需求** → 与用户交互，转换为项目文档
    //! 2. **Clarify Requirements** → Interact with user, convert to project docs
    //! 3. **调度分配** → 调用对应的执行Agent
    //! 3. **Schedule & Allocate** → Invoke corresponding execution Agents
    //! 4. **监控反馈** → 推送关键决策给人类
    //! 4. **Monitor Feedback** → Push critical decisions to humans
    //! 5. **验收汇报** → 更新TODO，生成报告
    //! 5. **Acceptance & Reporting** → Update TODOs, generate reports
    //!
    //! # Quick Start
    //!
    //! ```rust,ignore
    //! use mofa_sdk::secretary::{
    //!     AgentInfo, DefaultSecretaryBuilder, ChannelConnection, DefaultInput,
    //!     SecretaryOutput, TodoPriority,
    //! };
    //! use std::sync::Arc;
    //!
    //! #[tokio::main]
    //! async fn main() -> GlobalResult<()> {
    //!     // 1. 创建秘书Agent
    //!     // 1. Create Secretary Agent
    //!     let mut backend_agent = AgentInfo::new("backend_agent", "后端Agent");
    //!     backend_agent.capabilities = vec!["backend".to_string()];
    //!     backend_agent.current_load = 0;
    //!     backend_agent.available = true;
    //!     backend_agent.performance_score = 0.9;
    //!
    //!     let secretary = DefaultSecretaryBuilder::new()
    //!         .with_id("my_secretary")
    //!         .with_name("项目秘书")
    //!         .with_auto_clarify(true)
    //!         .with_executor(backend_agent)
    //!         .build()
    //!         .await;
    //!
    //!     // 2. 创建通道连接
    //!     // 2. Create channel connection
    //!     let (conn, input_tx, mut output_rx) = ChannelConnection::new_pair(32);
    //!
    //!     // 3. 启动事件循环
    //!     // 3. Start event loop
    //!     let handle = secretary.start(conn).await;
    //!
    //!     // 4. 发送用户输入
    //!     // 4. Send user input
    //!     input_tx.send(DefaultInput::Idea {
    //!         content: "开发一个REST API".to_string(),
    //!         priority: Some(TodoPriority::High),
    //!         metadata: None,
    //!     }).await?;
    //!
    //!     // 5. 处理秘书输出
    //!     // 5. Handle secretary output
    //!     while let Some(output) = output_rx.recv().await {
    //!         match output {
    //!             SecretaryOutput::Acknowledgment { message } => {
    //!                 info!("秘书: {}", message);
    //!             }
    //!             SecretaryOutput::DecisionRequired { decision } => {
    //!                 info!("需要决策: {}", decision.description);
    //!                 // 处理决策...
    //!                 // Handle decision...
    //!             }
    //!             SecretaryOutput::Report { report } => {
    //!                 info!("汇报: {}", report.content);
    //!             }
    //!             _ => {}
    //!         }
    //!     }
    //!
    //!     handle.await??;
    //!     Ok(())
    //! }
    //! ```
    //!
    //! # 自定义LLM Provider
    //! # Custom LLM Provider
    //!
    //! ```rust,ignore
    //! use mofa_sdk::secretary::{LLMProvider, ChatMessage};
    //! use std::sync::Arc;
    //!
    //! struct MyLLMProvider {
    //!     api_key: String,
    //! }
    //!
    //! #[async_trait::async_trait]
    //! impl LLMProvider for MyLLMProvider {
    //!     fn name(&self) -> &str { "my-llm" }
    //!
    //!     async fn chat(&self, messages: Vec<ChatMessage>) -> GlobalResult<String> {
    //!         // 调用你的LLM API
    //!         // Call your LLM API
    //!         Ok("LLM响应".to_string())
    //!     }
    //! }
    //!
    //! // 使用自定义LLM
    //! // Use custom LLM
    //! let llm = Arc::new(MyLLMProvider { api_key: "...".to_string() });
    //! let secretary = DefaultSecretaryBuilder::new()
    //!     .with_llm(llm)
    //!     .build()
    //!     .await;
    //! ```

    pub use mofa_foundation::secretary::*;
}

// Re-export React module from mofa-foundation (always available)
pub mod react {
    //! ReAct (Reasoning + Acting) 框架
    //! ReAct (Reasoning + Acting) Framework
    //!
    //! ReAct 是一种将推理和行动相结合的智能代理架构。
    //! ReAct is an intelligent agent architecture that combines reasoning and acting.
    //! 代理通过"思考-行动-观察"循环来解决问题。
    //! Agents solve problems through a "Think-Act-Observe" cycle.

    pub use mofa_foundation::react::*;
}

// Re-export collaboration module from mofa-foundation (always available)
pub mod collaboration {
    //! 自适应协作协议模块
    //! Adaptive Collaboration Protocol Module
    //!
    //! 提供多 Agent 自适应协作的标准协议实现，支持根据任务描述动态切换协作模式。
    //! Provides standard protocol implementations for multi-agent adaptive collaboration, supporting dynamic switching based on task descriptions.
    //!
    //! # 标准协议
    //! # Standard Protocols
    //!
    //! - `RequestResponseProtocol`: 请求-响应模式，适合数据处理任务
    //! - `RequestResponseProtocol`: Request-Response mode, suitable for data processing tasks
    //! - `PublishSubscribeProtocol`: 发布-订阅模式，适合创意生成任务
    //! - `PublishSubscribeProtocol`: Publish-Subscribe mode, suitable for creative generation tasks
    //! - `ConsensusProtocol`: 共识机制模式，适合决策制定任务
    //! - `ConsensusProtocol`: Consensus mechanism mode, suitable for decision-making tasks
    //! - `DebateProtocol`: 辩论模式，适合审查任务
    //! - `DebateProtocol`: Debate mode, suitable for review tasks
    //! - `ParallelProtocol`: 并行模式，适合分析任务
    //! - `ParallelProtocol`: Parallel mode, suitable for analysis tasks
    //!
    //! # 快速开始
    //! # Quick Start
    //!
    //! ```rust,ignore
    //! use mofa_sdk::collaboration::{
    //!     RequestResponseProtocol, PublishSubscribeProtocol, ConsensusProtocol,
    //!     LLMDrivenCollaborationManager,
    //! };
    //! use std::sync::Arc;
    //!
    //! #[tokio::main]
    //! async fn main() -> GlobalResult<()> {
    //!     let manager = LLMDrivenCollaborationManager::new("agent_001");
    //!
    //!     // 注册标准协议
    //!     // Register standard protocols
    //!     manager.register_protocol(Arc::new(RequestResponseProtocol::new("agent_001"))).await?;
    //!     manager.register_protocol(Arc::new(PublishSubscribeProtocol::new("agent_001"))).await?;
    //!     manager.register_protocol(Arc::new(ConsensusProtocol::new("agent_001"))).await?;
    //!
    //!     // 执行任务（使用自然语言描述，系统自动选择合适的协议）
    //!     // Execute task (describe in natural language, system auto-selects protocol)
    //!     let result = manager.execute_task(
    //!         "处理数据: [1, 2, 3]",  // 任务描述
    //!         serde_json::json!({"data": [1, 2, 3]})
    //!     ).await?;
    //!
    //!     println!("Result: {:?}", result);
    //!     Ok(())
    //! }
    //! ```

    pub use mofa_foundation::collaboration::*;
}

// =============================================================================
// Persistence module (re-export from mofa-foundation)
// =============================================================================

// Re-export Persistence module from mofa-foundation
pub mod persistence {
    pub use mofa_foundation::persistence::*;

    /// 快速创建带 PostgreSQL 持久化的 LLM Agent
    /// Quickly create LLM Agent with PostgreSQL persistence
    ///
    /// 自动处理：
    /// Automatic handling:
    /// - 数据库连接（从 DATABASE_URL）
    /// - DB Connection (from DATABASE_URL)
    /// - OpenAI Provider（从 OPENAI_API_KEY）
    /// - OpenAI Provider (from OPENAI_API_KEY)
    /// - 持久化插件
    /// - Persistence plugin
    /// - 自动生成 user_id、tenant_id、agent_id 和 session_id
    /// - Auto-generate user_id, tenant_id, agent_id, and session_id
    ///
    /// # 环境变量
    /// # Environment Variables
    /// - DATABASE_URL: PostgreSQL 连接字符串
    /// - DATABASE_URL: PostgreSQL connection string
    /// - OPENAI_API_KEY: OpenAI API 密钥
    /// - OPENAI_API_KEY: OpenAI API Key
    /// - USER_ID: 用户 ID（可选）
    /// - USER_ID: User ID (optional)
    /// - TENANT_ID: 租户 ID（可选）
    /// - TENANT_ID: Tenant ID (optional)
    /// - AGENT_ID: Agent ID（可选）
    /// - AGENT_ID: Agent ID (optional)
    /// - SESSION_ID: 会话 ID（可选）
    /// - SESSION_ID: Session ID (optional)
    ///
    /// # 示例
    /// # Example
    ///
    /// ```rust,ignore
    /// use mofa_sdk::persistence::quick_agent_with_postgres;
    ///
    /// #[tokio::main]
    /// async fn main() -> mofa_sdk::llm::LLMResult<()> {
    ///     let agent = quick_agent_with_postgres("你是一个有用的助手")
    ///         .await?
    ///         .with_name("聊天助手")
    ///         .build_async()
    ///         .await;
    ///     Ok(())
    /// }
    /// ```
    #[cfg(feature = "persistence-postgres")]
    pub async fn quick_agent_with_postgres(
        system_prompt: &str,
    ) -> Result<crate::llm::LLMAgentBuilder, crate::llm::LLMError> {
        use std::sync::Arc;

        // 1. 初始化数据库
        // 1. Initialize database
        let store_arc = PostgresStore::from_env()
            .await
            .map_err(|e| crate::llm::LLMError::Other(format!("数据库连接失败: {}", e)))?;

        // 2. 从环境变量获取或生成 IDs
        // 2. Get from env or generate IDs
        let user_id = std::env::var("USER_ID")
            .ok()
            .and_then(|s| uuid::Uuid::parse_str(&s).ok())
            .unwrap_or_else(uuid::Uuid::now_v7);

        let tenant_id = std::env::var("TENANT_ID")
            .ok()
            .and_then(|s| uuid::Uuid::parse_str(&s).ok())
            .unwrap_or_else(uuid::Uuid::now_v7);

        let agent_id = std::env::var("AGENT_ID")
            .ok()
            .and_then(|s| uuid::Uuid::parse_str(&s).ok())
            .unwrap_or_else(uuid::Uuid::now_v7);

        let session_id = std::env::var("SESSION_ID")
            .ok()
            .and_then(|s| uuid::Uuid::parse_str(&s).ok())
            .unwrap_or_else(uuid::Uuid::now_v7);

        // 3. 创建持久化插件（直接使用 Arc<PostgresStore> 作为存储）
        // 3. Create persistence plugin (using Arc<PostgresStore> directly)
        let plugin = PersistencePlugin::new(
            "persistence-plugin",
            store_arc.clone(),
            store_arc,
            user_id,
            tenant_id,
            agent_id,
            session_id,
        );

        // 4. 返回预配置的 builder
        // 4. Return pre-configured builder
        Ok(crate::llm::LLMAgentBuilder::from_env()?
            .with_system_prompt(system_prompt)
            .with_plugin(plugin))
    }

    /// 快速创建带内存持久化的 LLM Agent
    /// Quickly create LLM Agent with memory persistence
    ///
    /// 使用内存存储，适合测试和开发环境。
    /// Uses memory storage, suitable for test and dev environments.
    ///
    /// # 环境变量
    /// # Environment Variables
    /// - OPENAI_API_KEY: OpenAI API 密钥
    /// - OPENAI_API_KEY: OpenAI API Key
    ///
    /// # 示例
    /// # Example
    ///
    /// ```rust,ignore
    /// use mofa_sdk::persistence::quick_agent_with_memory;
    ///
    /// #[tokio::main]
    /// async fn main() -> mofa_sdk::llm::LLMResult<()> {
    ///     let agent = quick_agent_with_memory("你是一个有用的助手")
    ///         .await?
    ///         .with_name("聊天助手")
    ///         .build_async()
    ///         .await;
    ///     Ok(())
    /// }
    /// ```
    pub async fn quick_agent_with_memory(
        system_prompt: &str,
    ) -> Result<crate::llm::LLMAgentBuilder, crate::llm::LLMError> {
        let store = InMemoryStore::new();

        // 生成 IDs
        // Generate IDs
        let user_id = uuid::Uuid::now_v7();
        let tenant_id = uuid::Uuid::now_v7();
        let agent_id = uuid::Uuid::now_v7();
        let session_id = uuid::Uuid::now_v7();

        let plugin = PersistencePlugin::from_store(
            "persistence-plugin",
            store,
            user_id,
            tenant_id,
            agent_id,
            session_id,
        );

        Ok(crate::llm::LLMAgentBuilder::from_env()?
            .with_system_prompt(system_prompt)
            .with_plugin(plugin))
    }
}

// =============================================================================
// Messaging module (re-export from mofa-foundation)
// =============================================================================

// Re-export Messaging module from mofa-foundation
pub mod messaging {
    //! Generic message bus framework for decoupled agent architectures
    //!
    //! Provides:
    //! - Generic message types with pub/sub patterns
    //! - Inbound/outbound message separation
    //! - Trait-based message contracts
    //!
    //! # Quick Start
    //!
    //! ```rust,ignore
    //! use mofa_sdk::messaging::{MessageBus, SimpleInboundMessage, SimpleOutboundMessage};
    //!
    //! let bus = MessageBus::new(100);
    //!
    //! // Subscribe to inbound messages
    //! let mut rx = bus.subscribe_inbound();
    //!
    //! // Publish a message
    //! let msg = SimpleInboundMessage::new("telegram", "user", "chat", "Hello");
    //! bus.publish_inbound(msg).await?;
    //! ```

    pub use mofa_foundation::messaging::*;
}

// =============================================================================
// Dora-rs runtime support (enabled with `dora` feature)
// =============================================================================

#[cfg(feature = "dora")]
pub mod dora {
    //! Dora-rs adapter for distributed dataflow runtime
    //!
    //! This module provides MoFA framework integration with dora-rs, including:
    //! - DoraNode wrapper: Agent lifecycle management
    //! - DoraOperator wrapper: Plugin capability abstraction
    //! - DoraDataflow wrapper: Multi-agent collaborative dataflow
    //! - DoraChannel wrapper: Cross-agent communication channel
    //! - DoraRuntime wrapper: Complete runtime support (embedded/distributed)
    //!
    //! # Example
    //!
    //! ```rust,ignore
    //! use mofa_sdk::dora::{DoraRuntime, RuntimeConfig, run_dataflow};
    //!
    //! #[tokio::main]
    //! async fn main() -> eyre::Result<()> {
    //!     // Quick run with helper function
    //!     let result = run_dataflow("dataflow.yml").await?;
    //!     info!("Dataflow {} completed", result.uuid);
    //!
    //!     // Or use the builder pattern
    //!     let mut runtime = DoraRuntime::embedded("dataflow.yml");
    //!     let result = runtime.run().await?;
    //!     Ok(())
    //! }
    //! ```

    // Re-export dora adapter types
    pub use mofa_runtime::dora_adapter::*;

    // Re-export dora-specific runtime types from mofa_runtime root
    pub use mofa_runtime::{AgentBuilder, AgentRuntime, MoFARuntime};
}

// =============================================================================
// Agent Skills - Progressive Disclosure Skills System
// =============================================================================

// Module declaration for skills (public)
pub mod skills;

// Public skills module with re-exports
