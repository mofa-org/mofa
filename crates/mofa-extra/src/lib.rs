//! MoFA Extra 模块
//!
//! 提供 MoFA 框架的扩展功能，包括：
//! - Rhai 脚本引擎集成
//! - 动态工具系统
//! - 规则引擎
//! - 脚本化工作流节点

#[cfg(feature = "rhai-scripting")]
pub mod rhai;

#[cfg(feature = "rhai-scripting")]
pub use rhai::*;
