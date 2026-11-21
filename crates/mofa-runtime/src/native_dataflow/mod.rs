//! Native mofa-rs dataflow runtime.
//!
//! This module provides dataflow primitives built entirely on mofa-runtime's
//! own tokio channels and agent infrastructure:
//!
//! # Getting started
//!
//! ```ignore
//! use mofa_runtime::native_dataflow::{
//!     DataflowBuilder, NativeRuntime, NodeConfig,
//! };
//!
//! #[tokio::main]
//! async fn main() -> anyhow::Result<()> {
//!     let mut runtime = NativeRuntime::new();
//!
//!     let df = DataflowBuilder::new("my-pipeline")
//!         .add_node_config(NodeConfig { node_id: "producer".into(), outputs: vec!["out".into()], ..Default::default() })
//!         .add_node_config(NodeConfig { node_id: "consumer".into(), inputs:  vec!["in".into()],  ..Default::default() })
//!         .connect("producer", "out", "consumer", "in")
//!         .build()
//!         .await?;
//!
//!     runtime.register_dataflow(df).await?;
//!     runtime.start().await?;
//!     // ... drive workload ...
//!     runtime.stop().await?;
//!     Ok(())
//! }
//! ```

pub mod channel;
pub mod dataflow;
pub mod error;
pub mod node;
pub mod runtime;

// Flatten the most commonly used types into the module namespace.

pub use channel::{ChannelConfig, ChannelManager, MessageEnvelope, NativeChannel};
pub use dataflow::{
    DataflowBuilder, DataflowConfig, DataflowState, NativeDataflow, NodeConnection,
};
pub use error::{DataflowError, DataflowResult};
pub use node::{NativeNode, NodeConfig, NodeEventLoop, NodeState};
pub use runtime::{NativeRuntime, RuntimeState};
