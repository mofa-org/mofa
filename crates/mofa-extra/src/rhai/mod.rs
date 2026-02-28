//! Rhai 脚本引擎集成模块
//! Rhai script engine integration module
//!
//! 为 MoFA 框架提供 Rhai 嵌入式脚本支持，实现：
//! Provides Rhai embedded scripting support for the MoFA framework, achieving:
//! 1. 动态工作流节点脚本化
//! 1. Scripting of dynamic workflow nodes
//! 2. 条件判断和数据转换的脚本化定义
//! 2. Scripted definition of conditional logic and data transformation
//! 3. 动态工具定义
//! 3. Dynamic tool definitions
//! 4. 配置驱动的规则引擎
//! 4. Configuration-driven rule engine
//! 5. 热重载脚本支持
//! 5. Hot-reload script support

pub mod engine;
pub mod error;
pub mod rules;
pub mod tools;
pub mod workflow;

pub use engine::*;
pub use error::*;
pub use rules::*;
pub use tools::*;
pub use workflow::*;
