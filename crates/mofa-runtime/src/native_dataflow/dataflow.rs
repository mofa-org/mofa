//! Native dataflow graph: nodes connected by typed message channels.
//!
//! [`NativeDataflow`] owns a set of [`NativeNode`]s wired together via
//! [`NodeConnection`]s.  An internal router task dispatches messages from the
//! source node's output port to the target node's event queue.
//!
//! Typical usage:
//!
//! ```ignore
//! use mofa_runtime::native_dataflow::{DataflowBuilder, NodeConfig};
//!
//! let dataflow = DataflowBuilder::new("pipeline")
//!     .add_node_config(NodeConfig { node_id: "a".into(), outputs: vec!["out".into()], ..Default::default() })
//!     .add_node_config(NodeConfig { node_id: "b".into(), inputs: vec!["in".into()],  ..Default::default() })
//!     .connect("a", "out", "b", "in")
//!     .build_and_start()
//!     .await?;
//! ```

use crate::native_dataflow::error::{DataflowError, DataflowResult};
use crate::native_dataflow::node::{NativeNode, NodeConfig};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{Mutex, RwLock, mpsc};
use tokio::task::JoinHandle;
use tracing::{error, info};

/// Configuration for a [`NativeDataflow`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataflowConfig {
    /// Unique dataflow identifier (auto-generated if not set).
    pub dataflow_id: String,
    /// Human-readable name.
    pub name: String,
    /// Default channel buffer size used by the router.
    pub default_buffer_size: usize,
    /// Arbitrary key-value metadata.
    pub custom_config: HashMap<String, String>,
}

impl Default for DataflowConfig {
    fn default() -> Self {
        Self {
            dataflow_id: uuid::Uuid::now_v7().to_string(),
            name: "native_dataflow".to_string(),
            default_buffer_size: 1024,
            custom_config: HashMap::new(),
        }
    }
}

/// Lifecycle state of a [`NativeDataflow`].
#[derive(Debug, Clone, PartialEq)]
pub enum DataflowState {
    Created,
    Building,
    Ready,
    Running,
    Paused,
    Stopping,
    Stopped,
    Error(String),
}

/// Directed edge in the dataflow graph.
///
/// When the source node sends bytes on `source_output`, the router delivers
/// them to the target node's `target_input` event queue.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeConnection {
    pub source_node: String,
    pub source_output: String,
    pub target_node: String,
    pub target_input: String,
}

/// Internal router message carrying raw bytes from one node output.
#[derive(Debug)]
struct RouterMessage {
    source_node: String,
    source_output: String,
    data: Vec<u8>,
}

/// A native multi-agent dataflow graph.
pub struct NativeDataflow {
    config: DataflowConfig,
    state: Arc<RwLock<DataflowState>>,
    nodes: Arc<RwLock<HashMap<String, Arc<NativeNode>>>>,
    connections: Arc<RwLock<Vec<NodeConnection>>>,
    router_tx: mpsc::Sender<RouterMessage>,
    router_rx: Mutex<Option<mpsc::Receiver<RouterMessage>>>,
    router_handle: Mutex<Option<JoinHandle<()>>>,
}

impl NativeDataflow {
    /// Create a new dataflow from the given configuration.
    pub fn new(config: DataflowConfig) -> Self {
        let (router_tx, router_rx) = mpsc::channel(config.default_buffer_size);
        Self {
            config,
            state: Arc::new(RwLock::new(DataflowState::Created)),
            nodes: Arc::new(RwLock::new(HashMap::new())),
            connections: Arc::new(RwLock::new(Vec::new())),
            router_tx,
            router_rx: Mutex::new(Some(router_rx)),
            router_handle: Mutex::new(None),
        }
    }

    /// Return the dataflow configuration.
    pub fn config(&self) -> &DataflowConfig {
        &self.config
    }

    /// Return the current lifecycle state.
    pub async fn state(&self) -> DataflowState {
        self.state.read().await.clone()
    }

    /// Add a node.  Only allowed while the dataflow is in `Created` or
    /// `Building` state.
    pub async fn add_node(&self, node: NativeNode) -> DataflowResult<()> {
        {
            let state = self.state.read().await;
            if *state != DataflowState::Created && *state != DataflowState::Building {
                return Err(DataflowError::DataflowError(
                    "Cannot add nodes after dataflow is ready".to_string(),
                ));
            }
        }

        let node_id = node.config().node_id.clone();
        let mut nodes = self.nodes.write().await;
        if nodes.contains_key(&node_id) {
            return Err(DataflowError::DataflowError(format!(
                "Node '{}' already exists",
                node_id
            )));
        }
        nodes.insert(node_id.clone(), Arc::new(node));
        info!("Added node '{}' to dataflow '{}'", node_id, self.config.dataflow_id);
        Ok(())
    }

    /// Declare a directed connection from `source_node:source_output` to
    /// `target_node:target_input`.  Both nodes must have been added first.
    pub async fn connect(
        &self,
        source_node: &str,
        source_output: &str,
        target_node: &str,
        target_input: &str,
    ) -> DataflowResult<()> {
        {
            let state = self.state.read().await;
            if *state != DataflowState::Created && *state != DataflowState::Building {
                return Err(DataflowError::DataflowError(
                    "Cannot add connections after dataflow is ready".to_string(),
                ));
            }
        }

        let nodes = self.nodes.read().await;
        if !nodes.contains_key(source_node) {
            return Err(DataflowError::DataflowError(format!(
                "Source node '{}' not found",
                source_node
            )));
        }
        if !nodes.contains_key(target_node) {
            return Err(DataflowError::DataflowError(format!(
                "Target node '{}' not found",
                target_node
            )));
        }
        drop(nodes);

        let conn = NodeConnection {
            source_node: source_node.to_string(),
            source_output: source_output.to_string(),
            target_node: target_node.to_string(),
            target_input: target_input.to_string(),
        };
        self.connections.write().await.push(conn);
        info!(
            "Connected '{}:{}' -> '{}:{}'",
            source_node, source_output, target_node, target_input
        );
        Ok(())
    }

    /// Validate connections and transition to `Ready`.
    ///
    /// Must be called before [`start`].
    pub async fn build(&self) -> DataflowResult<()> {
        {
            let mut state = self.state.write().await;
            *state = DataflowState::Building;
        }

        let nodes = self.nodes.read().await;
        let connections = self.connections.read().await;
        for conn in connections.iter() {
            if !nodes.contains_key(&conn.source_node) {
                return Err(DataflowError::DataflowError(format!(
                    "Source node '{}' not found in connection",
                    conn.source_node
                )));
            }
            if !nodes.contains_key(&conn.target_node) {
                return Err(DataflowError::DataflowError(format!(
                    "Target node '{}' not found in connection",
                    conn.target_node
                )));
            }
        }
        drop(nodes);
        drop(connections);

        {
            let mut state = self.state.write().await;
            *state = DataflowState::Ready;
        }
        info!("Dataflow '{}' built successfully", self.config.dataflow_id);
        Ok(())
    }

    /// Start all nodes and the internal message router.
    pub async fn start(&self) -> DataflowResult<()> {
        {
            let state = self.state.read().await;
            if *state != DataflowState::Ready {
                return Err(DataflowError::DataflowError(
                    "Dataflow not ready — call build() first".to_string(),
                ));
            }
        }

        let nodes = self.nodes.read().await;
        for (id, node) in nodes.iter() {
            node.init().await.map_err(|e| {
                DataflowError::DataflowError(format!("Failed to init node '{}': {}", id, e))
            })?;
        }
        drop(nodes);

        {
            let mut state = self.state.write().await;
            *state = DataflowState::Running;
        }

        self.start_router().await;
        info!("Dataflow '{}' started", self.config.dataflow_id);
        Ok(())
    }

    /// Spawn the background router task.
    async fn start_router(&self) {
        let rx = match self.router_rx.lock().await.take() {
            Some(rx) => rx,
            None => {
                error!("Router receiver already consumed; start_router called twice");
                return;
            }
        };

        let connections = self.connections.clone();
        let nodes = self.nodes.clone();

        let handle = tokio::spawn(async move {
            let mut rx = rx;
            while let Some(msg) = rx.recv().await {
                let conns = connections.read().await;
                let node_map = nodes.read().await;

                for conn in conns.iter() {
                    if conn.source_node == msg.source_node
                        && conn.source_output == msg.source_output
                    {
                        if let Some(target) = node_map.get(&conn.target_node) {
                            let port = conn.target_input.clone();
                            if let Err(e) = target.inject_raw(port, msg.data.clone()).await {
                                error!(
                                    "Router failed to deliver to '{}': {}",
                                    conn.target_node, e
                                );
                            }
                        }
                    }
                }
            }
        });

        *self.router_handle.lock().await = Some(handle);
    }

    /// Return a reference-counted handle to a node by id.
    pub async fn get_node(&self, node_id: &str) -> Option<Arc<NativeNode>> {
        self.nodes.read().await.get(node_id).cloned()
    }

    /// Return all node identifiers in this dataflow.
    pub async fn node_ids(&self) -> Vec<String> {
        self.nodes.read().await.keys().cloned().collect()
    }

    /// Pause all nodes.
    pub async fn pause(&self) -> DataflowResult<()> {
        {
            let state = self.state.read().await;
            if *state != DataflowState::Running {
                return Err(DataflowError::DataflowError("Dataflow not running".to_string()));
            }
        }

        let nodes = self.nodes.read().await;
        for node in nodes.values() {
            node.pause().await?;
        }

        *self.state.write().await = DataflowState::Paused;
        info!("Dataflow '{}' paused", self.config.dataflow_id);
        Ok(())
    }

    /// Resume all paused nodes.
    pub async fn resume(&self) -> DataflowResult<()> {
        {
            let state = self.state.read().await;
            if *state != DataflowState::Paused {
                return Err(DataflowError::DataflowError("Dataflow not paused".to_string()));
            }
        }

        let nodes = self.nodes.read().await;
        for node in nodes.values() {
            node.resume().await?;
        }

        *self.state.write().await = DataflowState::Running;
        info!("Dataflow '{}' resumed", self.config.dataflow_id);
        Ok(())
    }

    /// Stop all nodes and abort the router task.
    pub async fn stop(&self) -> DataflowResult<()> {
        *self.state.write().await = DataflowState::Stopping;

        {
            let nodes = self.nodes.read().await;
            for node in nodes.values() {
                node.stop().await?;
            }
        }

        if let Some(handle) = self.router_handle.lock().await.take() {
            handle.abort();
        }

        *self.state.write().await = DataflowState::Stopped;
        info!("Dataflow '{}' stopped", self.config.dataflow_id);
        Ok(())
    }
}

/// Fluent builder for [`NativeDataflow`].
pub struct DataflowBuilder {
    config: DataflowConfig,
    nodes: Vec<NativeNode>,
    connections: Vec<NodeConnection>,
}

impl DataflowBuilder {
    /// Start building a dataflow with the given name.
    pub fn new(name: &str) -> Self {
        Self {
            config: DataflowConfig {
                name: name.to_string(),
                ..Default::default()
            },
            nodes: Vec::new(),
            connections: Vec::new(),
        }
    }

    /// Override the auto-generated dataflow ID.
    pub fn with_id(mut self, id: &str) -> Self {
        self.config.dataflow_id = id.to_string();
        self
    }

    /// Set the router channel buffer size.
    pub fn with_buffer_size(mut self, size: usize) -> Self {
        self.config.default_buffer_size = size;
        self
    }

    /// Add a pre-built node.
    pub fn add_node(mut self, node: NativeNode) -> Self {
        self.nodes.push(node);
        self
    }

    /// Add a node built from the given configuration.
    pub fn add_node_config(mut self, config: NodeConfig) -> Self {
        self.nodes.push(NativeNode::new(config));
        self
    }

    /// Add a directed edge from `source_node:source_output` to
    /// `target_node:target_input`.
    pub fn connect(
        mut self,
        source_node: &str,
        source_output: &str,
        target_node: &str,
        target_input: &str,
    ) -> Self {
        self.connections.push(NodeConnection {
            source_node: source_node.to_string(),
            source_output: source_output.to_string(),
            target_node: target_node.to_string(),
            target_input: target_input.to_string(),
        });
        self
    }

    /// Validate and build the dataflow without starting it.
    pub async fn build(self) -> DataflowResult<NativeDataflow> {
        let dataflow = NativeDataflow::new(self.config);
        for node in self.nodes {
            dataflow.add_node(node).await?;
        }
        for conn in self.connections {
            dataflow
                .connect(
                    &conn.source_node,
                    &conn.source_output,
                    &conn.target_node,
                    &conn.target_input,
                )
                .await?;
        }
        dataflow.build().await?;
        Ok(dataflow)
    }

    /// Build and immediately start the dataflow.
    pub async fn build_and_start(self) -> DataflowResult<NativeDataflow> {
        let dataflow = self.build().await?;
        dataflow.start().await?;
        Ok(dataflow)
    }
}

#[cfg(test)]
mod tests {
    use super::{DataflowBuilder, DataflowState, NodeConfig};
    use std::time::Duration;

    fn node(id: &str, inputs: &[&str], outputs: &[&str]) -> NodeConfig {
        NodeConfig {
            node_id: id.to_string(),
            inputs: inputs.iter().map(|s| s.to_string()).collect(),
            outputs: outputs.iter().map(|s| s.to_string()).collect(),
            ..Default::default()
        }
    }

    #[tokio::test]
    async fn test_builder_creates_ready_dataflow() {
        let df = DataflowBuilder::new("test")
            .add_node_config(node("a", &[], &["out"]))
            .add_node_config(node("b", &["in"], &[]))
            .connect("a", "out", "b", "in")
            .build()
            .await
            .unwrap();

        assert_eq!(df.state().await, DataflowState::Ready);
        assert_eq!(df.node_ids().await.len(), 2);
    }

    #[tokio::test]
    async fn test_full_lifecycle() {
        let df = DataflowBuilder::new("lifecycle")
            .add_node_config(node("n", &[], &[]))
            .build_and_start()
            .await
            .unwrap();

        assert_eq!(df.state().await, DataflowState::Running);

        df.pause().await.unwrap();
        assert_eq!(df.state().await, DataflowState::Paused);

        df.resume().await.unwrap();
        assert_eq!(df.state().await, DataflowState::Running);

        df.stop().await.unwrap();
        assert_eq!(df.state().await, DataflowState::Stopped);
    }

    /// Stopping a running dataflow must abort the router task promptly so
    /// resources are released and no second start attempt deadlocks.
    #[tokio::test]
    async fn test_stop_aborts_router_without_deadlock() {
        let df = DataflowBuilder::new("abort_test")
            .add_node_config(node("n", &[], &[]))
            .build_and_start()
            .await
            .unwrap();

        tokio::time::timeout(Duration::from_secs(2), df.stop())
            .await
            .expect("stop() timed out — router not aborted")
            .expect("stop() returned an error");

        assert_eq!(df.state().await, DataflowState::Stopped);
        assert!(
            df.router_handle.lock().await.is_none(),
            "router_handle should be None after stop()"
        );
    }

    #[tokio::test]
    async fn test_duplicate_node_rejected() {
        let df = DataflowBuilder::new("dup")
            .add_node_config(node("a", &[], &[]))
            .build()
            .await
            .unwrap();

        // Adding the same node id after build (in Stopped→Running state) should error.
        let extra = super::NativeNode::new(node("a", &[], &[]));
        assert!(df.add_node(extra).await.is_err());
    }

    #[tokio::test]
    async fn test_unknown_node_connection_rejected() {
        let result = DataflowBuilder::new("bad")
            .add_node_config(node("a", &[], &["out"]))
            .connect("a", "out", "missing", "in")
            .build()
            .await;

        assert!(result.is_err());
    }
}
