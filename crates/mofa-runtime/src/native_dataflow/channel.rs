//! Native inter-agent communication channel.
//!
//! [`NativeChannel`] supports three messaging patterns, all built on tokio
//! primitives:
//!
//! - **Point-to-point** (`send_p2p` / `receive_p2p`): directed messages to a
//!   specific registered agent.
//! - **Broadcast** (`broadcast` / `subscribe_broadcast`): fan-out to every
//!   current subscriber.
//! - **Pub-sub** (`publish` / `subscribe` / `subscribe_topic`): topic-scoped
//!   fan-out.

use crate::native_dataflow::error::{DataflowError, DataflowResult};
use mofa_kernel::message::AgentMessage;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{Mutex, RwLock, broadcast, mpsc};
use tokio::time::timeout;
use tracing::{debug, info};

type ReceiverMap = Arc<RwLock<HashMap<String, Arc<Mutex<mpsc::Receiver<MessageEnvelope>>>>>>;

/// Configuration for a [`NativeChannel`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelConfig {
    /// Unique channel identifier.
    pub channel_id: String,
    /// Capacity of internal mpsc/broadcast buffers.
    pub buffer_size: usize,
    /// Maximum time to wait for a message in blocking receive operations.
    pub message_timeout: Duration,
    /// Whether messages should be persisted (reserved for future use).
    pub persistent: bool,
}

impl Default for ChannelConfig {
    fn default() -> Self {
        Self {
            channel_id: uuid::Uuid::now_v7().to_string(),
            buffer_size: 1024,
            message_timeout: Duration::from_secs(30),
            persistent: false,
        }
    }
}

/// A message with routing metadata travelling through the native channel.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageEnvelope {
    /// Unique message identifier.
    pub message_id: String,
    /// Agent that produced the message.
    pub sender_id: String,
    /// Intended recipient for point-to-point delivery; `None` for broadcast.
    pub receiver_id: Option<String>,
    /// Topic for pub-sub routing; `None` for non-topic messages.
    pub topic: Option<String>,
    /// Unix-epoch timestamp in milliseconds.
    pub timestamp: u64,
    /// Raw message payload.
    pub payload: Vec<u8>,
    /// Arbitrary string metadata.
    pub metadata: HashMap<String, String>,
}

impl MessageEnvelope {
    /// Create a new envelope with the given sender and payload.
    pub fn new(sender_id: &str, payload: Vec<u8>) -> Self {
        Self {
            message_id: uuid::Uuid::now_v7().to_string(),
            sender_id: sender_id.to_string(),
            receiver_id: None,
            topic: None,
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64,
            payload,
            metadata: HashMap::new(),
        }
    }

    /// Set the point-to-point receiver.
    pub fn to(mut self, receiver_id: &str) -> Self {
        self.receiver_id = Some(receiver_id.to_string());
        self
    }

    /// Set the pub-sub topic.
    pub fn with_topic(mut self, topic: &str) -> Self {
        self.topic = Some(topic.to_string());
        self
    }

    /// Attach a metadata key-value pair.
    pub fn with_metadata(mut self, key: &str, value: &str) -> Self {
        self.metadata.insert(key.to_string(), value.to_string());
        self
    }

    /// Serialize an [`AgentMessage`] into an envelope.
    pub fn from_agent_message(
        sender_id: &str,
        message: &AgentMessage,
    ) -> DataflowResult<Self> {
        let payload = bincode::serialize(message)?;
        Ok(Self::new(sender_id, payload))
    }

    /// Deserialize the payload back into an [`AgentMessage`].
    pub fn to_agent_message(&self) -> DataflowResult<AgentMessage> {
        bincode::deserialize(&self.payload)
            .map_err(|e| DataflowError::DeserializationError(e.to_string()))
    }
}

/// Native channel hub supporting P2P, broadcast, and pub-sub messaging.
pub struct NativeChannel {
    config: ChannelConfig,
    p2p_senders: Arc<RwLock<HashMap<String, mpsc::Sender<MessageEnvelope>>>>,
    broadcast_tx: broadcast::Sender<MessageEnvelope>,
    topic_subscribers: Arc<RwLock<HashMap<String, Vec<String>>>>,
    topic_channels: Arc<RwLock<HashMap<String, broadcast::Sender<MessageEnvelope>>>>,
    receivers: ReceiverMap,
}

impl NativeChannel {
    /// Create a new channel with the given configuration.
    pub fn new(config: ChannelConfig) -> Self {
        let (broadcast_tx, _) = broadcast::channel(config.buffer_size);
        Self {
            config,
            p2p_senders: Arc::new(RwLock::new(HashMap::new())),
            broadcast_tx,
            topic_subscribers: Arc::new(RwLock::new(HashMap::new())),
            topic_channels: Arc::new(RwLock::new(HashMap::new())),
            receivers: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Return a reference to the channel configuration.
    pub fn config(&self) -> &ChannelConfig {
        &self.config
    }

    /// Register an agent so it can send and receive P2P messages.
    pub async fn register_agent(&self, agent_id: &str) -> DataflowResult<()> {
        let (tx, rx) = mpsc::channel(self.config.buffer_size);

        {
            let mut senders = self.p2p_senders.write().await;
            senders.insert(agent_id.to_string(), tx);
        }
        {
            let mut receivers = self.receivers.write().await;
            receivers.insert(agent_id.to_string(), Arc::new(Mutex::new(rx)));
        }

        info!("Agent '{}' registered to channel '{}'", agent_id, self.config.channel_id);
        Ok(())
    }

    /// Deregister an agent and remove it from all topic subscriptions.
    pub async fn unregister_agent(&self, agent_id: &str) -> DataflowResult<()> {
        {
            let mut senders = self.p2p_senders.write().await;
            senders.remove(agent_id);
        }
        {
            let mut receivers = self.receivers.write().await;
            receivers.remove(agent_id);
        }
        {
            let mut topic_subs = self.topic_subscribers.write().await;
            for subscribers in topic_subs.values_mut() {
                subscribers.retain(|id| id != agent_id);
            }
        }

        info!(
            "Agent '{}' unregistered from channel '{}'",
            agent_id, self.config.channel_id
        );
        Ok(())
    }

    /// Subscribe `agent_id` to the given topic, creating the topic channel if
    /// needed.
    pub async fn subscribe(&self, agent_id: &str, topic: &str) -> DataflowResult<()> {
        {
            let mut topic_subs = self.topic_subscribers.write().await;
            topic_subs
                .entry(topic.to_string())
                .or_default()
                .push(agent_id.to_string());
        }
        {
            let mut topic_channels = self.topic_channels.write().await;
            if !topic_channels.contains_key(topic) {
                let (tx, _) = broadcast::channel(self.config.buffer_size);
                topic_channels.insert(topic.to_string(), tx);
            }
        }

        debug!("Agent '{}' subscribed to topic '{}'", agent_id, topic);
        Ok(())
    }

    /// Remove `agent_id` from the given topic.
    pub async fn unsubscribe(&self, agent_id: &str, topic: &str) -> DataflowResult<()> {
        let mut topic_subs = self.topic_subscribers.write().await;
        if let Some(subscribers) = topic_subs.get_mut(topic) {
            subscribers.retain(|id| id != agent_id);
            if subscribers.is_empty() {
                topic_subs.remove(topic);
            }
        }
        debug!("Agent '{}' unsubscribed from topic '{}'", agent_id, topic);
        Ok(())
    }

    /// Send a point-to-point message to the receiver specified in the envelope.
    pub async fn send_p2p(&self, envelope: MessageEnvelope) -> DataflowResult<()> {
        let receiver_id = envelope
            .receiver_id
            .clone()
            .ok_or_else(|| DataflowError::ChannelError("No receiver specified for P2P".to_string()))?;

        let senders = self.p2p_senders.read().await;
        let tx = senders.get(&receiver_id).ok_or_else(|| {
            DataflowError::AgentNotFound(format!("Receiver '{}' not registered", receiver_id))
        })?;

        tx.send(envelope)
            .await
            .map_err(|e| DataflowError::ChannelError(e.to_string()))?;

        debug!("P2P message delivered to '{}'", receiver_id);
        Ok(())
    }

    /// Broadcast a message to all current broadcast subscribers.
    pub async fn broadcast(&self, envelope: MessageEnvelope) -> DataflowResult<()> {
        match self.broadcast_tx.send(envelope) {
            Ok(n) => debug!("Broadcast delivered to {} receivers", n),
            Err(_) => debug!("Broadcast: no active receivers"),
        }
        Ok(())
    }

    /// Publish a message to all subscribers of the topic set in the envelope.
    pub async fn publish(&self, envelope: MessageEnvelope) -> DataflowResult<()> {
        let topic = envelope
            .topic
            .clone()
            .ok_or_else(|| DataflowError::ChannelError("No topic specified for publish".to_string()))?;

        let topic_channels = self.topic_channels.read().await;
        let tx = topic_channels
            .get(&topic)
            .ok_or_else(|| DataflowError::ChannelError(format!("Topic '{}' not found", topic)))?;

        match tx.send(envelope) {
            Ok(n) => debug!("Published to topic '{}' with {} receivers", topic, n),
            Err(_) => debug!("Published to topic '{}' but no receivers", topic),
        }
        Ok(())
    }

    /// Blocking receive on the P2P queue of `agent_id`.
    ///
    /// Returns `Err(DataflowError::Timeout)` if no message arrives within the
    /// channel's configured timeout.
    pub async fn receive_p2p(
        &self,
        agent_id: &str,
    ) -> DataflowResult<Option<MessageEnvelope>> {
        let rx = {
            let receivers = self.receivers.read().await;
            receivers.get(agent_id).cloned().ok_or_else(|| {
                DataflowError::AgentNotFound(format!("Agent '{}' not registered", agent_id))
            })?
        };

        let mut guard = rx.lock().await;
        match timeout(self.config.message_timeout, guard.recv()).await {
            Ok(Some(env)) => Ok(Some(env)),
            Ok(None) => Ok(None),
            Err(_) => Err(DataflowError::Timeout("receive_p2p timed out".to_string())),
        }
    }

    /// Non-blocking poll on the P2P queue of `agent_id`.
    pub async fn try_receive_p2p(
        &self,
        agent_id: &str,
    ) -> DataflowResult<Option<MessageEnvelope>> {
        let rx = {
            let receivers = self.receivers.read().await;
            receivers.get(agent_id).cloned().ok_or_else(|| {
                DataflowError::AgentNotFound(format!("Agent '{}' not registered", agent_id))
            })?
        };

        let mut guard = rx.lock().await;
        match guard.try_recv() {
            Ok(env) => Ok(Some(env)),
            Err(mpsc::error::TryRecvError::Empty) => Ok(None),
            Err(mpsc::error::TryRecvError::Disconnected) => {
                Err(DataflowError::ChannelError("Channel disconnected".to_string()))
            }
        }
    }

    /// Subscribe to the global broadcast stream.
    pub fn subscribe_broadcast(&self) -> broadcast::Receiver<MessageEnvelope> {
        self.broadcast_tx.subscribe()
    }

    /// Subscribe to a topic broadcast stream.
    ///
    /// The topic must have been created via [`subscribe`] first.
    pub async fn subscribe_topic(
        &self,
        topic: &str,
    ) -> DataflowResult<broadcast::Receiver<MessageEnvelope>> {
        let topic_channels = self.topic_channels.read().await;
        let tx = topic_channels
            .get(topic)
            .ok_or_else(|| DataflowError::ChannelError(format!("Topic '{}' not found", topic)))?;
        Ok(tx.subscribe())
    }

    /// Return the list of agents currently subscribed to `topic`.
    pub async fn topic_subscribers(&self, topic: &str) -> Vec<String> {
        let subs = self.topic_subscribers.read().await;
        subs.get(topic).cloned().unwrap_or_default()
    }

    /// Return the list of registered agent identifiers.
    pub async fn registered_agents(&self) -> Vec<String> {
        let senders = self.p2p_senders.read().await;
        senders.keys().cloned().collect()
    }
}

/// Manages a collection of named [`NativeChannel`]s.
pub struct ChannelManager {
    channels: Arc<RwLock<HashMap<String, Arc<NativeChannel>>>>,
    default_config: ChannelConfig,
}

impl ChannelManager {
    /// Create a new empty manager.
    pub fn new() -> Self {
        Self {
            channels: Arc::new(RwLock::new(HashMap::new())),
            default_config: ChannelConfig::default(),
        }
    }

    /// Return an existing channel or create one with `channel_id`.
    pub async fn get_or_create(&self, channel_id: &str) -> Arc<NativeChannel> {
        {
            let channels = self.channels.read().await;
            if let Some(ch) = channels.get(channel_id) {
                return ch.clone();
            }
        }

        let config = ChannelConfig {
            channel_id: channel_id.to_string(),
            ..self.default_config.clone()
        };
        let channel = Arc::new(NativeChannel::new(config));

        let mut channels = self.channels.write().await;
        channels.insert(channel_id.to_string(), channel.clone());
        channel
    }

    /// Remove a channel, returning it if it existed.
    pub async fn remove(&self, channel_id: &str) -> Option<Arc<NativeChannel>> {
        let mut channels = self.channels.write().await;
        channels.remove(channel_id)
    }

    /// Return all registered channel identifiers.
    pub async fn channel_ids(&self) -> Vec<String> {
        let channels = self.channels.read().await;
        channels.keys().cloned().collect()
    }
}

impl Default for ChannelManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::{ChannelConfig, ChannelManager, MessageEnvelope, NativeChannel};
    use std::sync::Arc;
    use std::time::Duration;

    #[tokio::test]
    async fn test_p2p_send_receive() {
        let ch = NativeChannel::new(ChannelConfig::default());
        ch.register_agent("a").await.unwrap();
        ch.register_agent("b").await.unwrap();

        let env = MessageEnvelope::new("a", b"hello".to_vec()).to("b");
        ch.send_p2p(env).await.unwrap();

        let received = ch.try_receive_p2p("b").await.unwrap();
        assert!(received.is_some());
        assert_eq!(received.unwrap().payload, b"hello");
    }

    #[tokio::test]
    async fn test_pubsub() {
        let ch = NativeChannel::new(ChannelConfig::default());
        ch.register_agent("pub").await.unwrap();
        ch.register_agent("sub").await.unwrap();
        ch.subscribe("sub", "events").await.unwrap();

        let mut rx = ch.subscribe_topic("events").await.unwrap();

        let env = MessageEnvelope::new("pub", b"data".to_vec()).with_topic("events");
        ch.publish(env).await.unwrap();

        let msg = rx.recv().await.unwrap();
        assert_eq!(msg.payload, b"data");
    }

    #[tokio::test]
    async fn test_broadcast() {
        let ch = NativeChannel::new(ChannelConfig::default());
        let mut rx = ch.subscribe_broadcast();

        let env = MessageEnvelope::new("src", b"broadcast".to_vec());
        ch.broadcast(env).await.unwrap();

        let msg = rx.recv().await.unwrap();
        assert_eq!(msg.payload, b"broadcast");
    }

    #[tokio::test]
    async fn test_channel_manager_deduplication() {
        let mgr = ChannelManager::new();
        let c1 = mgr.get_or_create("ch1").await;
        let c2 = mgr.get_or_create("ch1").await;
        assert_eq!(c1.config().channel_id, c2.config().channel_id);
        assert_eq!(mgr.channel_ids().await.len(), 1);
    }

    /// Regression: `register_agent` must not be blocked by a concurrent
    /// `receive_p2p` on a different agent.
    #[tokio::test]
    async fn test_register_does_not_deadlock_during_receive() {
        let ch = Arc::new(NativeChannel::new(ChannelConfig {
            message_timeout: Duration::from_millis(500),
            ..ChannelConfig::default()
        }));
        ch.register_agent("reader").await.unwrap();

        let ch2 = ch.clone();
        let reader_task = tokio::spawn(async move {
            let _ = ch2.receive_p2p("reader").await;
        });
        tokio::time::sleep(Duration::from_millis(20)).await;

        let start = std::time::Instant::now();
        ch.register_agent("new_agent").await.unwrap();
        let elapsed = start.elapsed();

        reader_task.await.unwrap();
        assert!(
            elapsed < Duration::from_millis(400),
            "register_agent was blocked for {:?}",
            elapsed
        );
    }
}
