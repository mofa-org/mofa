//! 秘书Agent模式 - 可扩展的智能助手框架实现
//! Secretary Agent Pattern - Extensible intelligent assistant framework implementation
//!
//! 本模块提供秘书Agent的具体实现，核心抽象定义在 `mofa_kernel::agent::secretary` 中。
//! This module provides concrete implementations of the Secretary Agent, with core abstractions defined in `mofa_kernel::agent::secretary`.
//!
//! ## 架构
//! ## Architecture
//!
//! - **mofa-kernel**: 提供核心抽象 (`SecretaryBehavior`, `UserConnection`, `SecretaryContext` 等)
//! - **mofa-kernel**: Provides core abstractions (`SecretaryBehavior`, `UserConnection`, `SecretaryContext`, etc.)
//! - **mofa-foundation**: 提供具体实现 (`DefaultSecretaryBehavior`, `SecretaryCore`, 组件等)
//! - **mofa-foundation**: Provides concrete implementations (`DefaultSecretaryBehavior`, `SecretaryCore`, components, etc.)
//!
//! ## 使用方式
//! ## Usage
//!
//! ### 方式1: 使用默认实现
//! ### Method 1: Using the default implementation
//!
//! ```rust,ignore
//! use mofa_foundation::secretary::{
//!     SecretaryCore, ChannelConnection,
//!     DefaultSecretaryBuilder, DefaultInput, DefaultOutput,
//! };
//!
//! // 创建默认秘书行为
//! // Create default secretary behavior
//! let behavior = DefaultSecretaryBuilder::new()
//!     .with_name("我的秘书")
//!     .with_auto_clarify(true)
//!     .build();
//!
//! // 创建核心引擎
//! // Create core engine
//! let core = SecretaryCore::new(behavior);
//!
//! // 创建连接
//! // Create connection
//! let (conn, input_tx, output_rx) = ChannelConnection::new_pair(32);
//!
//! // 启动秘书
//! // Start the secretary
//! let (handle, join) = core.start(conn).await;
//! ```
//!
//! ### 方式2: 自定义秘书行为
//! ### Method 2: Customizing secretary behavior
//!
//! ```rust,ignore
//! use mofa_kernel::agent::secretary::{SecretaryBehavior, SecretaryContext};
//! use mofa_foundation::secretary::SecretaryCore;
//!
//! struct MySecretary { /* ... */ }
//!
//! #[async_trait]
//! impl SecretaryBehavior for MySecretary {
//!     type Input = MyInput;
//!     type Output = MyOutput;
//!     type State = MyState;
//!
//!     async fn handle_input(
//!         &self,
//!         input: Self::Input,
//!         ctx: &mut SecretaryContext<Self::State>,
//!     ) -> GlobalResult<Vec<Self::Output>> {
//!         // 自定义处理逻辑
//!         // Custom processing logic
//!     }
//!
//!     fn initial_state(&self) -> Self::State {
//!         MyState::new()
//!     }
//! }
//! ```

// =============================================================================
// 实现模块
// Implementation Modules
// =============================================================================

use mofa_kernel::agent::types::error::{GlobalError, GlobalResult};
mod agent_router;
mod connection;
mod core;
mod llm;

/// 默认实现模块
/// Default implementation module
pub mod default;
pub mod monitoring;

// =============================================================================
// 从 mofa-kernel 重新导出核心抽象
// Re-export core abstractions from mofa-kernel
// =============================================================================

// 核心 traits (来自 mofa-kernel)
// Core traits (from mofa-kernel)
pub use mofa_kernel::agent::secretary::{
    // Traits
    ConnectionFactory,
    EventListener,
    InputHandler,
    Middleware,
    PhaseHandler,
    PhaseResult,
    SecretaryBehavior,
    SecretaryContext,
    SecretaryContextBuilder,
    SecretaryEvent,
    SecretaryInput,
    SecretaryOutput,
    SharedSecretaryContext,
    UserConnection,
    WorkflowOrchestrator,
    WorkflowResult,
};

// =============================================================================
// 本模块实现
// Local module implementations
// =============================================================================

// 核心引擎实现 (SecretaryCore 在此模块实现，使用 kernel 的抽象)
// Core engine implementation (SecretaryCore implemented here using kernel abstractions)
pub use core::{CoreState, SecretaryCoreConfig, SecretaryHandle};
pub use core::{SecretaryCore, SecretaryCoreBuilder};

// 连接实现 (Foundation)
// Connection implementations (Foundation)
pub use connection::{ChannelConnection, TimeoutConnection};

// LLM 抽象
// LLM abstractions
pub use llm::{
    ChatMessage, ConversationHistory, LLMProvider, ModelInfo, extract_json_block, parse_llm_json,
};

// Agent路由
// Agent Routing
pub use agent_router::{
    AgentInfo, AgentProvider, AgentRouter, CapabilityRouter, CompositeRouter, ConditionLogic,
    InMemoryAgentProvider, LLMAgentRouter, RoutingContext, RoutingDecision, RoutingDecisionType,
    RoutingRule, RuleBasedRouter, RuleCondition, RuleField, RuleOperator,
};

// =============================================================================
// 便捷导出（默认实现类型）
// Convenience exports (Default implementation types)
// =============================================================================

// 从default模块重新导出常用类型
// Re-export commonly used types from the default module
pub use default::{
    // 类型
    // Types
    Artifact,
    ClarificationQuestion,
    ClarificationStrategy,
    CriticalDecision,
    DecisionOption,
    DecisionType,
    DefaultInput,
    DefaultOutput,
    DefaultSecretaryBehavior,
    DefaultSecretaryBuilder,
    DispatchResult,
    DispatchStrategy,
    ExecutionResult,
    HumanResponse,
    MonitorEvent,
    ProjectRequirement,
    QueryType,
    Report,
    ReportConfig,
    ReportFormat,
    ReportType,
    // 组件
    // Components
    Reporter,
    RequirementClarifier,
    Resource,
    SecretaryCommand,
    SecretaryMessage,
    Subtask,
    TaskCoordinator,
    TaskExecutionStatus,
    TaskMonitor,
    TaskSnapshot,
    TodoItem,
    TodoManager,
    TodoPriority,
    // 默认行为
    // Default behaviors
    TodoStatus,
    WorkPhase,
};

// =============================================================================
// Prelude 模块
// Prelude Module
// =============================================================================

/// 常用导出的 prelude 模块
/// Prelude module for common exports
pub mod prelude {
    pub use super::{
        // 核心 traits (来自 kernel)
        // Core traits (from kernel)
        AgentInfo,
        AgentProvider,
        AgentRouter,
        // 核心组件 (实现在此模块)
        // Core components (implemented in this module)
        ChannelConnection,
        ChatMessage,
        LLMProvider,
        SecretaryBehavior,
        // 连接 (来自 kernel)
        // Connection (from kernel)
        SecretaryContext,
        SecretaryContextBuilder,
        // LLM
        SecretaryCore,
        SecretaryCoreBuilder,
        // Agent
        SecretaryInput,
        SecretaryOutput,
        UserConnection,
    };
}
