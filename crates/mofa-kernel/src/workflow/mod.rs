//! Workflow Module
//!
//! This module provides the core workflow/graph abstractions for MoFA.
//! Inspired by LangGraph, it provides a stateful graph-based workflow system
//! with support for:
//!
//! - **Reducer Pattern**: Configurable state update strategies (overwrite, append, merge, etc.)
//! - **Command Pattern**: Unified state updates and control flow
//! - **Send Pattern**: Dynamic edge creation for MapReduce scenarios
//! - **RemainingSteps**: Active recursion limit tracking
//!
//! # Architecture
//!
//! This module defines traits only (kernel layer). Concrete implementations
//! are provided in `mofa-foundation`.
//!
//! # Example
//!
//! ```rust,ignore
//! use mofa_kernel::workflow::{StateGraph, Command, RuntimeContext, START, END};
//!
//! // Define state
//! #[derive(Clone, Serialize, Deserialize)]
//! struct MyState {
//!     messages: Vec<String>,
//! }
//!
//! impl GraphState for MyState {
//!     // ... implementation
//! }
//!
//! // Build graph
//! let graph = StateGraphImpl::<MyState>::new("my_workflow")
//!     .add_node("process", Box::new(ProcessNode))
//!     .add_edge(START, "process")
//!     .add_edge("process", END)
//!     .compile()?;
//!
//! // Execute
//! let result = graph.invoke(initial_state, None).await?;
//! ```

pub mod command;
pub mod context;
pub mod graph;
pub mod reducer;
pub mod state;

// Re-export public API
pub use command::{Command, ControlFlow, SendCommand};
pub use context::{GraphConfig, RemainingSteps, RuntimeContext};
pub use graph::{CompiledGraph, EdgeTarget, NodeFunc, StateGraph, StreamEvent, StepResult, END, START};
pub use reducer::{Reducer, ReducerType, StateUpdate};
pub use state::{GraphState, JsonState, StateSchema};
