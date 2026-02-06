//! Agent 注册中心
//!
//! 运行时层直接复用 mofa-kernel 的注册中心实现，避免重复与漂移

pub use mofa_kernel::agent::registry::*;
