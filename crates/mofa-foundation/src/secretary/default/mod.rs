//! 默认实现模块
//! Default implementation module
//!
//! 提供秘书框架的默认实现，开发者可以直接使用或作为参考。
//! Provides default implementations of the secretary framework for direct use or reference.
//!
//! ## 包含内容
//! ## Contents
//!
//! - [`DefaultSecretaryBehavior`]: 默认的秘书行为实现（5阶段工作流）
//! - [`DefaultSecretaryBehavior`]: Default secretary behavior implementation (5-phase workflow)
//! - [`TodoManager`]: 任务管理器
//! - [`TodoManager`]: Task manager
//! - [`RequirementClarifier`]: 需求澄清器
//! - [`RequirementClarifier`]: Requirement clarifier
//! - [`TaskCoordinator`]: 任务协调器
//! - [`TaskCoordinator`]: Task coordinator
//! - [`TaskMonitor`]: 任务监控器
//! - [`TaskMonitor`]: Task monitor
//! - [`Reporter`]: 汇报生成器
//! - [`Reporter`]: Report generator
//!
//! ## 使用方式
//! ## Usage
//!
//! 开发者有两种选择：
//! Developers have two options:
//!
//! 1. **直接使用默认实现**
//! 1. **Use the default implementation directly**
//!
//! ```rust,ignore
//! use mofa_foundation::secretary::default::{
//!     DefaultSecretaryBehavior,
//!     DefaultSecretaryBuilder,
//! };
//!
//! let behavior = DefaultSecretaryBuilder::new()
//!     .with_name("我的秘书")
//!     .with_llm(my_llm)
//!     .build();
//!
//! let core = SecretaryCore::new(behavior);
//! ```
//!
//! 2. **自定义实现，复用部分组件**
//! 2. **Custom implementation, reusing specific components**
//!
//! ```rust,ignore
//! use mofa_foundation::secretary::{
//!     SecretaryBehavior, SecretaryContext,
//!     default::{TodoManager, RequirementClarifier},
//! };
//!
//! struct MySecretary {
//!     todo_manager: TodoManager,
//!     clarifier: RequirementClarifier,
//! }
//!
//! impl SecretaryBehavior for MySecretary {
//!     // 自定义实现，可以使用默认组件
//!     // Custom implementation using default components
//! }
//! ```

mod behavior;
mod clarifier;
mod coordinator;
mod monitor;
mod reporter;
mod todo;
pub mod types;

// 导出默认行为
// Export default behavior
pub use behavior::{DefaultSecretaryBehavior, DefaultSecretaryBuilder};

// 导出默认类型
// Export default types
pub use types::{
    // 任务类型
    // Task types
    Artifact,
    CriticalDecision,
    DecisionOption,
    DecisionType,
    DefaultInput,
    DefaultOutput,
    ExecutionResult,
    HumanResponse,
    ProjectRequirement,
    QueryType,
    Report,
    ReportType,
    Resource,
    SecretaryCommand,
    SecretaryMessage,
    Subtask,
    // 输入输出类型
    // Input and output types
    TaskExecutionStatus,
    TodoItem,
    TodoPriority,
    TodoStatus,
    WorkPhase,
};

// 导出组件
// Export components
pub use clarifier::{
    ClarificationQuestion, ClarificationSession, ClarificationStrategy, QuestionType,
    RequirementClarifier,
};
pub use coordinator::{DispatchResult, DispatchStrategy, TaskCoordinator};
pub use monitor::{MonitorEvent, TaskMonitor, TaskSnapshot};
pub use reporter::{ReportConfig, ReportFormat, Reporter};
pub use todo::TodoManager;
