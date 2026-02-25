use anyhow::Result;
use mofa_kernel::bus::{AgentBus, CommunicationMode};
use mofa_kernel::message::AgentMessage;
use std::sync::Arc;
use tokio::sync::RwLock;

/// A lightweight wrapper around `AgentBus` for capturing and asserting messages.
#[derive(Clone)]
pub struct MockAgentBus {
    pub inner: AgentBus,
    /// History of all captured messages for assertion
    pub captured_messages: Arc<RwLock<Vec<(String, CommunicationMode, AgentMessage)>>>,
}

impl MockAgentBus {
    pub async fn new() -> Result<Self> {
        Ok(Self {
            inner: AgentBus::new().await?,
            captured_messages: Arc::new(RwLock::new(Vec::new())),
        })
    }

    /// Simulate sending a message and capture it simultaneously
    pub async fn send_and_capture(
        &self,
        sender_id: &str,
        mode: CommunicationMode,
        message: AgentMessage,
    ) -> Result<()> {
        self.captured_messages
            .write()
            .await
            .push((sender_id.to_string(), mode.clone(), message.clone()));

        self.inner.send_message(sender_id, mode, &message).await
    }

    /// Clears the captured message history.
    pub async fn clear_history(&self) {
        self.captured_messages.write().await.clear();
    }
}
