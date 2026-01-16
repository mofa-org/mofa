//! 统一工具系统
//!
//! 提供工具注册、发现、适配、热加载等功能

pub mod adapters;
pub mod registry;

pub use adapters::{ClosureTool, FunctionTool};
pub use registry::UnifiedToolRegistry;
