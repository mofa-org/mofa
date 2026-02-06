//! 秘书Agent抽象层
//!
//! 秘书Agent是一种特殊的Agent模式，提供持续在线的交互式助手能力。
//! 本模块定义了秘书Agent的核心抽象，具体实现在 mofa-foundation 中。
//!
//! ## 设计理念
//!
//! - **核心抽象层**：框架只提供最核心的抽象与协议
//! - **行为可插拔**：通过 `SecretaryBehavior` trait 定义秘书行为
//! - **连接可扩展**：通过 `UserConnection` trait 支持多种通信方式
//!
//! ## 核心组件
//!
//! - [`SecretaryBehavior`]: 秘书行为trait，开发者实现此trait定义秘书逻辑
//! - [`SecretaryContext`]: 秘书上下文
//! - [`UserConnection`]: 用户连接抽象
//!
//! ## 使用方式
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
//!
//! // 创建并启动秘书 (Foundation 层提供具体引擎)
//! let core = SecretaryCore::new(MySecretary::new());
//! let (handle, join) = core.start(connection).await;
//! ```

mod connection;
mod context;
mod traits;

// 核心导出
pub use connection::{ConnectionFactory, UserConnection};
pub use context::{SecretaryContext, SecretaryContextBuilder, SharedSecretaryContext};
pub use traits::{
    EventListener, InputHandler, Middleware, PhaseHandler, PhaseResult, SecretaryBehavior,
    SecretaryEvent, SecretaryInput, SecretaryOutput, WorkflowOrchestrator, WorkflowResult,
};

/// Prelude 模块
pub mod prelude {
    pub use super::{
        PhaseHandler, PhaseResult, SecretaryBehavior, SecretaryContext, SecretaryInput,
        SecretaryOutput, UserConnection, WorkflowOrchestrator, WorkflowResult,
    };
    pub use async_trait::async_trait;
}
