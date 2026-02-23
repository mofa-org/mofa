//! MoFA Extra 模块
//! MoFA Extra Module
//!
//! 提供 MoFA 框架的扩展功能，包括：
//! Provides extended functionality for the MoFA framework, including:
//! - Rhai 脚本引擎集成
//! - Rhai script engine integration
//! - 动态工具系统
//! - Dynamic tool system
//! - 规则引擎
//! - Rule engine
//! - 脚本化工作流节点
//! - Scripted workflow nodes

#[cfg(feature = "rhai-scripting")]
pub mod rhai;

#[cfg(feature = "rhai-scripting")]
pub use rhai::*;
