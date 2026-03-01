//! 秘书Agent抽象层
//! Secretary Agent Abstraction Layer
//!
//! 秘书Agent是一种特殊的Agent模式，提供持续在线的交互式助手能力。
//! Secretary Agent is a special Agent pattern providing continuous online interactive assistant capabilities.
//! 本模块定义了秘书Agent的核心抽象，具体实现在 mofa-foundation 中。
//! This module defines the core abstractions of Secretary Agent, with concrete implementations in mofa-foundation.
//!
//! ## 设计理念
//! ## Design Philosophy
//!
//! - **核心抽象层**：框架只提供最核心的抽象与协议
//! - **Core Abstraction Layer**: The framework only provides the most essential abstractions and protocols
//! - **行为可插拔**：通过 `SecretaryBehavior` trait 定义秘书行为
//! - **Pluggable Behavior**: Secretary behavior is defined via the `SecretaryBehavior` trait
//! - **连接可扩展**：通过 `UserConnection` trait 支持多种通信方式
//! - **Extensible Connection**: Multiple communication methods are supported via the `UserConnection` trait
//!
//! ## 核心组件
//! ## Core Components
//!
//! - [`SecretaryBehavior`][]: 秘书行为trait，开发者实现此trait定义秘书逻辑
//! - [`SecretaryBehavior`][]: Secretary behavior trait, implemented by developers to define secretary logic
//! - [`SecretaryContext`][]: 秘书上下文
//! - [`SecretaryContext`][]: Secretary context
//! - [`UserConnection`][]: 用户连接抽象
//! - [`UserConnection`][]: User connection abstraction
//!
//! ## 使用方式
//! ## Usage
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
//!     ) -> Result<Vec<Self::Output>, SecretaryError> {
//!         // 自定义处理逻辑
//!         // Custom processing logic
//!     }
//!
//!     fn initial_state(&self) -> Self::State {
//!         MyState::new()
//!     }
//! }
//!
//! // 创建并启动秘书 (Foundation 层提供具体引擎)
//! // Create and start the secretary (Foundation layer provides the concrete engine)
//! let core = SecretaryCore::new(MySecretary::new());
//! let (handle, join) = core.start(connection).await;
//! ```

mod connection;
mod context;
pub mod error;
mod traits;

// 核心导出
// Core exports
pub use connection::{ConnectionFactory, UserConnection};
pub use context::{SecretaryContext, SecretaryContextBuilder, SharedSecretaryContext};
pub use error::{ConnectionError, SecretaryError};
pub use traits::{
    EventListener, InputHandler, Middleware, PhaseHandler, PhaseResult, SecretaryBehavior,
    SecretaryEvent, SecretaryInput, SecretaryOutput, WorkflowOrchestrator, WorkflowResult,
};

/// Prelude 模块
/// Prelude Module
pub mod prelude {
    pub use super::{
        PhaseHandler, PhaseResult, SecretaryBehavior, SecretaryContext, SecretaryInput,
        SecretaryOutput, UserConnection, WorkflowOrchestrator, WorkflowResult,
    };
    pub use async_trait::async_trait;
}
