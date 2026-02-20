//! Graph-based Workflow Orchestration
//!
//! 提供基于有向图的工作流编排系统，支持：
//! - 多种节点类型（任务、条件、并行、聚合、循环）
//! - DAG 拓扑排序执行
//! - 并行执行与同步
//! - 状态管理与数据传递
//! - 错误处理与重试
//! - 检查点与恢复
//! - DSL (YAML/TOML) 配置支持
//!
//! # StateGraph API (LangGraph-inspired)
//!
//! 新的 StateGraph API 提供了更直观的工作流构建方式：
//!
//! ```rust,ignore
//! use mofa_foundation::workflow::{StateGraphImpl, AppendReducer, OverwriteReducer};
//! use mofa_kernel::workflow::{StateGraph, START, END};
//!
//! let graph = StateGraphImpl::<MyState>::new("my_workflow")
//!     .add_reducer("messages", Box::new(AppendReducer))
//!     .add_node("process", Box::new(ProcessNode))
//!     .add_edge(START, "process")
//!     .add_edge("process", END)
//!     .compile()?;
//!
//! let result = graph.invoke(initial_state, None).await?;
//! ```

mod builder;
mod executor;
mod graph;
mod node;
mod reducers;
mod state;
mod state_graph;

pub mod dsl;

// Re-export kernel workflow types for convenience
pub use mofa_kernel::workflow::{
    Command, CompiledGraph, ControlFlow, GraphConfig, GraphState, JsonState, NodeFunc,
    Reducer, ReducerType, RemainingSteps, RuntimeContext, SendCommand, StateSchema, StateUpdate,
    END, START,
};

// Re-export kernel StateGraph trait
pub use mofa_kernel::workflow::StateGraph;

// Foundation-specific exports
pub use builder::*;
pub use dsl::*;
pub use executor::*;
pub use graph::*;
pub use node::*;
pub use reducers::*;
pub use state::*;
pub use state_graph::{CompiledGraphImpl, StateGraphImpl};
