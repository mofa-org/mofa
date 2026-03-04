//! Mock message bus for inspecting agent-to-agent communication.

use mofa_kernel::bus::{AgentBus, BusError, CommunicationMode};
use mofa_kernel::message::AgentMessage;
use std::sync::Arc;
use tokio::sync::RwLock;

/// A message-bus spy that records all sent messages for later assertions.
#[derive(Clone)]
pub struct MockAgentBus {
    /// The real bus implementation (delegates actual routing)
    pub inner: AgentBus,
    /// Chronologically ordered capture of `(sender_id, mode, message)`
    pub captured_messages: Arc<RwLock<Vec<(String, CommunicationMode, AgentMessage)>>>,
}

impl MockAgentBus {
    /// Create a new mock bus backed by a real [`AgentBus`].
    pub fn new() -> Self {
        Self {
            inner: AgentBus::new(),
            captured_messages: Arc::new(RwLock::new(Vec::new())),
        }
    }

    /// Send a message through the inner bus **and** record it.
    pub async fn send_and_capture(
        &self,
        sender_id: &str,
        mode: CommunicationMode,
        message: AgentMessage,
    ) -> Result<(), BusError> {
        // Record first so assertions see the message even if send fails
        self.captured_messages
            .write()
            .await
            .push((sender_id.to_string(), mode.clone(), message.clone()));

        self.inner.send_message(sender_id, mode, &message).await
    }

    /// Number of messages captured so far.
    pub async fn message_count(&self) -> usize {
        self.captured_messages.read().await.len()
    }

    /// Clears the capture history.
    pub async fn clear_history(&self) {
        self.captured_messages.write().await.clear();
    }
}

impl Default for MockAgentBus {
    fn default() -> Self {
        Self::new()
    }
}
