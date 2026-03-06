//! Native in-process runtime for dataflow execution.
//!
//! [`NativeRuntime`] ties together one or more [`NativeDataflow`]s and a
//! shared [`NativeChannel`], providing a single start/stop surface without
//! any Dora-rs dependency.
//!
//! # Example
//!
//! ```ignore
//! use mofa_runtime::native_dataflow::{NativeRuntime, DataflowBuilder, NodeConfig};
//!
//! #[tokio::main]
//! async fn main() -> anyhow::Result<()> {
//!     let mut runtime = NativeRuntime::new();
//!
//!     let df = DataflowBuilder::new("pipeline")
//!         .add_node_config(NodeConfig { node_id: "producer".into(), outputs: vec!["out".into()], ..Default::default() })
//!         .add_node_config(NodeConfig { node_id: "consumer".into(), inputs: vec!["in".into()],  ..Default::default() })
//!         .connect("producer", "out", "consumer", "in")
//!         .build()
//!         .await?;
//!
//!     runtime.register_dataflow(df).await?;
//!     runtime.start().await?;
//!     // ... run workload ...
//!     runtime.stop().await?;
//!     Ok(())
//! }
//! ```

use crate::native_dataflow::channel::{ChannelConfig, NativeChannel};
use crate::native_dataflow::dataflow::NativeDataflow;
use crate::native_dataflow::error::{DataflowError, DataflowResult};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::info;

/// Lifecycle state of the native runtime.
#[derive(Debug, Clone, PartialEq)]
pub enum RuntimeState {
    /// Runtime created but not yet started.
    Idle,
    /// All registered dataflows are running.
    Running,
    /// Runtime has been stopped.
    Stopped,
}

/// In-process runtime that manages a collection of [`NativeDataflow`]s and a
/// shared [`NativeChannel`] for cross-flow communication.
pub struct NativeRuntime {
    dataflows: Arc<RwLock<HashMap<String, Arc<NativeDataflow>>>>,
    channel: Arc<NativeChannel>,
    state: Arc<RwLock<RuntimeState>>,
}

impl NativeRuntime {
    /// Create a new runtime with a default shared channel.
    pub fn new() -> Self {
        Self {
            dataflows: Arc::new(RwLock::new(HashMap::new())),
            channel: Arc::new(NativeChannel::new(ChannelConfig::default())),
            state: Arc::new(RwLock::new(RuntimeState::Idle)),
        }
    }

    /// Create a runtime that uses the supplied channel configuration.
    pub fn with_channel_config(config: ChannelConfig) -> Self {
        Self {
            dataflows: Arc::new(RwLock::new(HashMap::new())),
            channel: Arc::new(NativeChannel::new(config)),
            state: Arc::new(RwLock::new(RuntimeState::Idle)),
        }
    }

    /// Return the current runtime state.
    pub async fn state(&self) -> RuntimeState {
        self.state.read().await.clone()
    }

    /// Return a reference-counted handle to the shared channel.
    pub fn channel(&self) -> Arc<NativeChannel> {
        self.channel.clone()
    }

    /// Register a dataflow with the runtime.
    ///
    /// Dataflows can be registered before or after the runtime is started.
    /// If the runtime is already running the dataflow will be started
    /// immediately.
    pub async fn register_dataflow(&self, dataflow: NativeDataflow) -> DataflowResult<String> {
        let id = dataflow.config().dataflow_id.clone();

        let already_running = *self.state.read().await == RuntimeState::Running;

        {
            let mut dataflows = self.dataflows.write().await;
            if dataflows.contains_key(&id) {
                return Err(DataflowError::DataflowError(format!(
                    "Dataflow '{}' already registered",
                    id
                )));
            }
            dataflows.insert(id.clone(), Arc::new(dataflow));
        }

        if already_running {
            let dataflows = self.dataflows.read().await;
            if let Some(df) = dataflows.get(&id) {
                df.start().await?;
            }
        }

        info!("Registered dataflow '{}'", id);
        Ok(id)
    }

    /// Return a reference-counted handle to a registered dataflow.
    pub async fn get_dataflow(&self, id: &str) -> Option<Arc<NativeDataflow>> {
        self.dataflows.read().await.get(id).cloned()
    }

    /// Return all registered dataflow identifiers.
    pub async fn dataflow_ids(&self) -> Vec<String> {
        self.dataflows.read().await.keys().cloned().collect()
    }

    /// Start all registered dataflows.
    ///
    /// Returns an error if the runtime is already running or if any dataflow
    /// fails to start (in which case the already-started dataflows are stopped
    /// before returning).
    pub async fn start(&self) -> DataflowResult<()> {
        {
            let state = self.state.read().await;
            if *state == RuntimeState::Running {
                return Err(DataflowError::DataflowError(
                    "Runtime already running".to_string(),
                ));
            }
        }

        let dataflows = self.dataflows.read().await;
        let mut started: Vec<Arc<NativeDataflow>> = Vec::new();

        for (id, df) in dataflows.iter() {
            if let Err(e) = df.start().await {
                // Roll back already-started dataflows.
                for started_df in &started {
                    let _ = started_df.stop().await;
                }
                return Err(DataflowError::DataflowError(format!(
                    "Failed to start dataflow '{}': {}",
                    id, e
                )));
            }
            started.push(df.clone());
        }

        *self.state.write().await = RuntimeState::Running;
        info!("NativeRuntime started with {} dataflow(s)", started.len());
        Ok(())
    }

    /// Stop all running dataflows.
    pub async fn stop(&self) -> DataflowResult<()> {
        {
            let state = self.state.read().await;
            if *state != RuntimeState::Running {
                return Err(DataflowError::DataflowError(
                    "Runtime is not running".to_string(),
                ));
            }
        }

        let dataflows = self.dataflows.read().await;
        for (id, df) in dataflows.iter() {
            if let Err(e) = df.stop().await {
                tracing::warn!("Error stopping dataflow '{}': {}", id, e);
            }
        }

        *self.state.write().await = RuntimeState::Stopped;
        info!("NativeRuntime stopped");
        Ok(())
    }
}

impl Default for NativeRuntime {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::{NativeRuntime, RuntimeState};
    use crate::native_dataflow::dataflow::DataflowBuilder;
    use crate::native_dataflow::node::NodeConfig;

    fn simple_dataflow(name: &str) -> impl std::future::Future<Output = crate::native_dataflow::error::DataflowResult<crate::native_dataflow::dataflow::NativeDataflow>> {
        let name = name.to_string();
        async move {
            DataflowBuilder::new(&name)
                .add_node_config(NodeConfig {
                    node_id: format!("{}_node", name),
                    ..Default::default()
                })
                .build()
                .await
        }
    }

    #[tokio::test]
    async fn test_start_stop_lifecycle() {
        let runtime = NativeRuntime::new();
        assert_eq!(runtime.state().await, RuntimeState::Idle);

        let df = simple_dataflow("df1").await.unwrap();
        runtime.register_dataflow(df).await.unwrap();

        runtime.start().await.unwrap();
        assert_eq!(runtime.state().await, RuntimeState::Running);

        runtime.stop().await.unwrap();
        assert_eq!(runtime.state().await, RuntimeState::Stopped);
    }

    #[tokio::test]
    async fn test_double_start_fails() {
        let runtime = NativeRuntime::new();
        let df = simple_dataflow("df2").await.unwrap();
        runtime.register_dataflow(df).await.unwrap();

        runtime.start().await.unwrap();
        assert!(runtime.start().await.is_err());
        runtime.stop().await.unwrap();
    }

    #[tokio::test]
    async fn test_duplicate_dataflow_rejected() {
        let runtime = NativeRuntime::new();
        let df1 = DataflowBuilder::new("dup")
            .with_id("same-id")
            .add_node_config(NodeConfig::default())
            .build()
            .await
            .unwrap();
        let df2 = DataflowBuilder::new("dup2")
            .with_id("same-id")
            .add_node_config(NodeConfig::default())
            .build()
            .await
            .unwrap();

        runtime.register_dataflow(df1).await.unwrap();
        assert!(runtime.register_dataflow(df2).await.is_err());
    }

    #[tokio::test]
    async fn test_get_dataflow() {
        let runtime = NativeRuntime::new();
        let df = DataflowBuilder::new("findme")
            .with_id("my-id")
            .add_node_config(NodeConfig::default())
            .build()
            .await
            .unwrap();

        runtime.register_dataflow(df).await.unwrap();

        let found = runtime.get_dataflow("my-id").await;
        assert!(found.is_some());

        let not_found = runtime.get_dataflow("no-such-id").await;
        assert!(not_found.is_none());
    }
}
