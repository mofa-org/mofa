//! Graph-based Workflow Orchestration
//!
//! 提供基于有向图的工作流编排系统，支持：
//! - 多种节点类型（任务、条件、并行、聚合、循环）
//! - DAG 拓扑排序执行
//! - 并行执行与同步
//! - 状态管理与数据传递
//! - 错误处理与重试
//! - 检查点与恢复

mod builder;
mod executor;
mod graph;
mod node;
mod state;

pub use builder::*;
pub use executor::*;
pub use graph::*;
pub use node::*;
pub use state::*;
