//! 工具系统 (Foundation 层)
//!
//! 提供工具适配器、内置工具与统一注册中心等具体实现。
//! Kernel 仅定义 Tool 接口与基础类型；具体实现放在 Foundation 层。

pub mod adapters;
pub mod registry;

pub use adapters::{BuiltinTools, ClosureTool, FunctionTool};
pub use registry::{ToolSearcher, UnifiedToolRegistry};
