//! Native agent node for dataflow execution.
//!
//! `NativeNode` wraps the mofa-runtime's own tokio channels and interrupt
//! mechanism to provide agent lifecycle management without any Dora-rs
//! dependency.

use crate::interrupt::AgentInterrupt;
use crate::native_dataflow::error::{DataflowError, DataflowResult};
use mofa_kernel::message::{AgentEvent, AgentMessage, TaskPriority, TaskRequest};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{RwLock, mpsc};
use tracing::{debug, info};

/// Configuration for a native dataflow node.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeConfig {
    /// Unique node identifier.
    pub node_id: String,
    /// Human-readable node name.
    pub name: String,
    /// List of input port names this node accepts.
    pub inputs: Vec<String>,
    /// List of output port names this node produces.
    pub outputs: Vec<String>,
    /// Capacity of the internal event buffer.
    pub event_buffer_size: usize,
    /// Default timeout for blocking receive operations.
    pub default_timeout: Duration,
    /// Arbitrary key-value metadata.
    pub custom_config: HashMap<String, String>,
}

impl Default for NodeConfig {
    fn default() -> Self {
        Self {
            node_id: uuid::Uuid::now_v7().to_string(),
            name: "native_node".to_string(),
            inputs: vec![],
            outputs: vec![],
            event_buffer_size: 1024,
            default_timeout: Duration::from_secs(30),
            custom_config: HashMap::new(),
        }
    }
}

/// Lifecycle state of a native node.
#[derive(Debug, Clone, PartialEq)]
pub enum NodeState {
    Created,
    Initializing,
    Running,
    Paused,
    Stopping,
    Stopped,
    Error(String),
}

/// An agent node in the native dataflow graph.
///
/// Each node manages its own event queue and output channels.  The
/// [`NodeEventLoop`] returned by [`NativeNode::create_event_loop`] can be
/// used by agent logic to receive events in a background task.
pub struct NativeNode {
    config: NodeConfig,
    state: Arc<RwLock<NodeState>>,
    interrupt: AgentInterrupt,
    event_tx: mpsc::Sender<AgentEvent>,
    event_rx: Arc<RwLock<mpsc::Receiver<AgentEvent>>>,
    output_channels: Arc<RwLock<HashMap<String, mpsc::Sender<Vec<u8>>>>>,
}

impl NativeNode {
    /// Create a new node from the given configuration.
    pub fn new(config: NodeConfig) -> Self {
        let (event_tx, event_rx) = mpsc::channel(config.event_buffer_size);
        Self {
            config,
            state: Arc::new(RwLock::new(NodeState::Created)),
            interrupt: AgentInterrupt::new(),
            event_tx,
            event_rx: Arc::new(RwLock::new(event_rx)),
            output_channels: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Return a reference to the node configuration.
    pub fn config(&self) -> &NodeConfig {
        &self.config
    }

    /// Return the current lifecycle state.
    pub async fn state(&self) -> NodeState {
        self.state.read().await.clone()
    }

    /// Return the interrupt handle for this node.
    pub fn interrupt(&self) -> &AgentInterrupt {
        &self.interrupt
    }

    /// Initialize the node and transition to `Running`.
    ///
    /// This sets up the output port channels so that downstream nodes can be
    /// connected to them after [`NativeDataflow::build`] wires the connections.
    pub async fn init(&self) -> DataflowResult<()> {
        let mut state = self.state.write().await;
        if *state != NodeState::Created {
            return Err(DataflowError::NodeInitError(
                "Node already initialized".to_string(),
            ));
        }
        *state = NodeState::Initializing;

        let mut output_channels = self.output_channels.write().await;
        for output in &self.config.outputs {
            let (tx, _rx) = mpsc::channel(self.config.event_buffer_size);
            output_channels.insert(output.clone(), tx);
        }

        *state = NodeState::Running;
        info!("NativeNode {} initialized", self.config.node_id);
        Ok(())
    }

    /// Send raw bytes to the named output port.
    pub async fn send_output(&self, output_id: &str, data: Vec<u8>) -> DataflowResult<()> {
        let state = self.state.read().await;
        if *state != NodeState::Running {
            return Err(DataflowError::NodeNotRunning);
        }
        drop(state);

        let output_channels = self.output_channels.read().await;
        if let Some(tx) = output_channels.get(output_id) {
            tx.send(data)
                .await
                .map_err(|e| DataflowError::ChannelError(e.to_string()))?;
            debug!("NativeNode {} sent data on output '{}'", self.config.node_id, output_id);
        } else {
            debug!(
                "NativeNode {}: output '{}' has no registered receiver; dropping",
                self.config.node_id, output_id
            );
        }
        Ok(())
    }

    /// Serialize and send an [`AgentMessage`] on the named output port.
    pub async fn send_message(
        &self,
        output_id: &str,
        message: &AgentMessage,
    ) -> DataflowResult<()> {
        let data = bincode::serialize(message)?;
        self.send_output(output_id, data).await
    }

    /// Inject an event directly into this node's event queue.
    ///
    /// This is used by the dataflow router when routing messages from upstream
    /// nodes.
    pub async fn inject_event(&self, event: AgentEvent) -> DataflowResult<()> {
        self.event_tx
            .send(event)
            .await
            .map_err(|e| DataflowError::ChannelError(e.to_string()))
    }

    /// Inject a raw byte payload as a `Custom` event.
    ///
    /// The router uses this to deliver messages that couldn't be deserialized
    /// into a known event type.
    pub async fn inject_raw(&self, port: String, data: Vec<u8>) -> DataflowResult<()> {
        // Try to deserialize as TaskRequest first, then AgentMessage.
        let event = if let Ok(task) = bincode::deserialize::<TaskRequest>(&data) {
            AgentEvent::TaskReceived(task)
        } else if let Ok(msg) = bincode::deserialize::<AgentMessage>(&data) {
            match msg {
                AgentMessage::Event(ev) => ev,
                AgentMessage::TaskRequest { task_id, content } => {
                    AgentEvent::TaskReceived(TaskRequest {
                        task_id,
                        content,
                        priority: TaskPriority::Medium,
                        deadline: None,
                        metadata: HashMap::new(),
                    })
                }
                _ => AgentEvent::Custom(port, data),
            }
        } else {
            AgentEvent::Custom(port, data)
        };
        self.inject_event(event).await
    }

    /// Register an external mpsc sender as a receiver for the named output port.
    ///
    /// The dataflow router calls this during `build()` to wire connections
    /// between nodes.
    pub async fn register_output_channel(
        &self,
        output_id: String,
        sender: mpsc::Sender<Vec<u8>>,
    ) -> DataflowResult<()> {
        let mut output_channels = self.output_channels.write().await;
        output_channels.insert(output_id, sender);
        Ok(())
    }

    /// Pause the node (suspends event delivery).
    pub async fn pause(&self) -> DataflowResult<()> {
        let mut state = self.state.write().await;
        if *state == NodeState::Running {
            *state = NodeState::Paused;
            info!("NativeNode {} paused", self.config.node_id);
        }
        Ok(())
    }

    /// Resume a paused node.
    pub async fn resume(&self) -> DataflowResult<()> {
        let mut state = self.state.write().await;
        if *state == NodeState::Paused {
            *state = NodeState::Running;
            info!("NativeNode {} resumed", self.config.node_id);
        }
        Ok(())
    }

    /// Stop the node and trigger its interrupt.
    pub async fn stop(&self) -> DataflowResult<()> {
        let mut state = self.state.write().await;
        *state = NodeState::Stopping;
        self.interrupt.trigger();
        *state = NodeState::Stopped;
        info!("NativeNode {} stopped", self.config.node_id);
        Ok(())
    }

    /// Create an event loop handle for this node.
    ///
    /// Spawn a Tokio task and drive the returned [`NodeEventLoop`] to process
    /// events produced by the router.
    pub fn create_event_loop(&self) -> NodeEventLoop {
        NodeEventLoop {
            event_rx: self.event_rx.clone(),
            interrupt: self.interrupt.clone(),
            state: self.state.clone(),
        }
    }
}

/// Event loop handle for a [`NativeNode`].
///
/// Typical usage inside a `tokio::spawn` block:
///
/// ```ignore
/// let el = node.create_event_loop();
/// tokio::spawn(async move {
///     while let Some(event) = el.next_event().await {
///         // process event ...
///         if matches!(event, AgentEvent::Shutdown) { break; }
///     }
/// });
/// ```
pub struct NodeEventLoop {
    event_rx: Arc<RwLock<mpsc::Receiver<AgentEvent>>>,
    interrupt: AgentInterrupt,
    state: Arc<RwLock<NodeState>>,
}

impl NodeEventLoop {
    /// Block until the next event arrives or the node is stopped.
    ///
    /// Returns `Some(AgentEvent::Shutdown)` when the node is stopping.
    pub async fn next_event(&self) -> Option<AgentEvent> {
        if self.interrupt.check() {
            return Some(AgentEvent::Shutdown);
        }

        let state = self.state.read().await;
        if *state == NodeState::Stopped || *state == NodeState::Stopping {
            return Some(AgentEvent::Shutdown);
        }
        drop(state);

        let mut event_rx = self.event_rx.write().await;
        tokio::select! {
            event = event_rx.recv() => event,
            _ = self.interrupt.notify.notified() => Some(AgentEvent::Shutdown),
        }
    }

    /// Non-blocking poll for the next event.
    pub async fn try_next_event(&self) -> Option<AgentEvent> {
        if self.interrupt.check() {
            return Some(AgentEvent::Shutdown);
        }
        let mut event_rx = self.event_rx.write().await;
        event_rx.try_recv().ok()
    }

    /// Returns `true` if the interrupt has been triggered.
    pub fn should_interrupt(&self) -> bool {
        self.interrupt.check()
    }

    /// Return the interrupt handle.
    pub fn interrupt(&self) -> &AgentInterrupt {
        &self.interrupt
    }
}

#[cfg(test)]
mod tests {
    use super::{NativeNode, NodeConfig, NodeState};

    #[tokio::test]
    async fn test_node_lifecycle() {
        let config = NodeConfig {
            node_id: "n1".to_string(),
            name: "Test".to_string(),
            outputs: vec!["out".to_string()],
            ..Default::default()
        };
        let node = NativeNode::new(config);
        assert_eq!(node.state().await, NodeState::Created);

        node.init().await.unwrap();
        assert_eq!(node.state().await, NodeState::Running);

        node.pause().await.unwrap();
        assert_eq!(node.state().await, NodeState::Paused);

        node.resume().await.unwrap();
        assert_eq!(node.state().await, NodeState::Running);

        node.stop().await.unwrap();
        assert_eq!(node.state().await, NodeState::Stopped);
    }

    #[tokio::test]
    async fn test_double_init_fails() {
        let node = NativeNode::new(NodeConfig::default());
        node.init().await.unwrap();
        assert!(node.init().await.is_err());
    }

    #[tokio::test]
    async fn test_inject_and_receive_event() {
        use mofa_kernel::message::AgentEvent;

        let node = NativeNode::new(NodeConfig::default());
        node.init().await.unwrap();
        let el = node.create_event_loop();

        node.inject_event(AgentEvent::Custom("test".to_string(), b"ping".to_vec()))
            .await
            .unwrap();

        let ev = el.try_next_event().await;
        assert!(matches!(ev, Some(AgentEvent::Custom(ref p, _)) if p == "test"));
    }
}
