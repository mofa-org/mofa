//! 秘书Agent模式 - 可扩展的智能助手框架实现
//!
//! 本模块提供秘书Agent的具体实现，核心抽象定义在 `mofa_kernel::agent::secretary` 中。
//!
//! ## 架构
//!
//! - **mofa-kernel**: 提供核心抽象 (`SecretaryBehavior`, `UserConnection`, `SecretaryContext` 等)
//! - **mofa-foundation**: 提供具体实现 (`DefaultSecretaryBehavior`, `SecretaryCore`, 组件等)
//!
//! ## 使用方式
//!
//! ### 方式1: 使用默认实现
//!
//! ```rust,ignore
//! use mofa_foundation::secretary::{
//!     SecretaryCore, ChannelConnection,
//!     DefaultSecretaryBuilder, DefaultInput, DefaultOutput,
//! };
//!
//! // 创建默认秘书行为
//! let behavior = DefaultSecretaryBuilder::new()
//!     .with_name("我的秘书")
//!     .with_auto_clarify(true)
//!     .build();
//!
//! // 创建核心引擎
//! let core = SecretaryCore::new(behavior);
//!
//! // 创建连接
//! let (conn, input_tx, output_rx) = ChannelConnection::new_pair(32);
//!
//! // 启动秘书
//! let (handle, join) = core.start(conn).await;
//! ```
//!
//! ### 方式2: 自定义秘书行为
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
//!     ) -> anyhow::Result<Vec<Self::Output>> {
//!         // 自定义处理逻辑
//!     }
//!
//!     fn initial_state(&self) -> Self::State {
//!         MyState::new()
//!     }
//! }
//! ```

// =============================================================================
// 实现模块
// =============================================================================

mod agent_router;
mod connection;
mod core;
mod llm;

/// 默认实现模块
pub mod default;
pub mod monitoring;

// =============================================================================
// 从 mofa-kernel 重新导出核心抽象
// =============================================================================

// 核心 traits (来自 mofa-kernel)
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
// =============================================================================

// 核心引擎实现 (SecretaryCore 在此模块实现，使用 kernel 的抽象)
pub use core::{CoreState, SecretaryCoreConfig, SecretaryHandle};
pub use core::{SecretaryCore, SecretaryCoreBuilder};

// 连接实现 (Foundation)
pub use connection::{ChannelConnection, TimeoutConnection};

// LLM 抽象
pub use llm::{
    ChatMessage, ConversationHistory, LLMProvider, ModelInfo, extract_json_block, parse_llm_json,
};

// Agent路由
pub use agent_router::{
    AgentInfo, AgentProvider, AgentRouter, CapabilityRouter, CompositeRouter, ConditionLogic,
    InMemoryAgentProvider, LLMAgentRouter, RoutingContext, RoutingDecision, RoutingDecisionType,
    RoutingRule, RuleBasedRouter, RuleCondition, RuleField, RuleOperator,
};

// =============================================================================
// 便捷导出（默认实现类型）
// =============================================================================

// 从default模块重新导出常用类型
pub use default::{
    // 类型
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
    TodoStatus,
    WorkPhase,
};

// =============================================================================
// Prelude 模块
// =============================================================================

/// 常用导出的 prelude 模块
pub mod prelude {
    pub use super::{
        // 核心 traits (来自 kernel)
        AgentInfo,
        AgentProvider,
        AgentRouter,
        // 核心组件 (实现在此模块)
        ChannelConnection,
        ChatMessage,
        LLMProvider,
        SecretaryBehavior,
        // 连接 (来自 kernel)
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
