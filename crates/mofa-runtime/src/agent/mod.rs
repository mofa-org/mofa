//! Agent 模块
//!
//! 重新导出 mofa-kernel 的 agent 模块，并添加 runtime 特定的功能

// 从 mofa-kernel 重新导出核心模块
pub use mofa_kernel::agent::capabilities;
pub use mofa_kernel::agent::context;
pub use mofa_kernel::agent::core;
pub use mofa_kernel::agent::error;
pub use mofa_kernel::agent::traits;
pub use mofa_kernel::agent::types;

// Runtime 特定的模块
pub mod config;
pub mod execution;
pub mod plugins;
pub mod registry;

// 重新导出常用类型
pub use execution::{ExecutionEngine, ExecutionOptions, ExecutionResult, ExecutionStatus};
pub use registry::{AgentFactory, AgentRegistry, RegistryStats};
