pub mod backpressure;
pub mod error;
pub mod metrics;

pub use backpressure::{BusConfig, ChannelConfig, LagPolicy};
pub use error::BusError;
pub use metrics::{BusMetrics, MetricsSnapshot};

use backpressure::LagPolicy as LagPolicyEnum;

use crate::agent::AgentMetadata;
use crate::message::AgentMessage;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tokio::sync::{RwLock, broadcast};
use tracing::warn;

/// Communication mode enumeration.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum CommunicationMode {
    /// Point-to-point communication (Single sender -> Single receiver).
    PointToPoint(String),
    /// Broadcast communication (Single sender -> All agents).
    Broadcast,
    /// Pub-Sub communication (Topic-based).
    PubSub(String),
}

pub type AgentChannelMap =
    Arc<RwLock<HashMap<String, HashMap<CommunicationMode, broadcast::Sender<Vec<u8>>>>>>;

/// Core structure for the communication bus.
///
/// Supports configurable buffer sizes, per-channel backpressure policies,
/// and observable metrics. See [`BusConfig`] for configuration options.
///
/// # Example
///
/// ```rust,ignore
/// use mofa_kernel::bus::{AgentBus, BusConfig, ChannelConfig, LagPolicy};
///
/// // High-throughput bus with skip-and-continue for lag
/// let config = BusConfig::new(
///     ChannelConfig::new(4096).with_lag_policy(LagPolicy::SkipAndContinue),
/// );
/// let bus = AgentBus::with_config(config);
///
/// // Access metrics
/// let snap = bus.metrics().snapshot();
/// println!("Delivery rate: {:.1}%", snap.delivery_rate() * 100.0);
/// ```
#[derive(Clone)]
pub struct AgentBus {
    /// Agent-to-communication channel mapping.
    agent_channels: AgentChannelMap,
    /// Topic-to-subscriber mapping (Exclusive to PubSub mode).
    topic_subscribers: Arc<RwLock<HashMap<String, HashSet<String>>>>,
    /// Global broadcast channel.
    broadcast_channel: broadcast::Sender<Vec<u8>>,
    /// Bus configuration (buffer sizes, lag policies).
    config: Arc<BusConfig>,
    /// Shared metrics (lock-free atomic counters).
    bus_metrics: Arc<BusMetrics>,
}

impl AgentBus {
    /// Create a communication bus instance with default configuration.
    ///
    /// Uses [`BusConfig::default()`] which sets a 256-slot buffer and
    /// [`LagPolicy::Error`] (lag is surfaced, never silent).
    pub fn new() -> Self {
        Self::with_config(BusConfig::default())
    }

    /// Create a bus with the given configuration.
    pub fn with_config(config: BusConfig) -> Self {
        let buffer_size = config.broadcast().buffer_size();
        let (broadcast_sender, _) = broadcast::channel(buffer_size);
        Self {
            agent_channels: Arc::new(RwLock::new(HashMap::new())),
            topic_subscribers: Arc::new(RwLock::new(HashMap::new())),
            broadcast_channel: broadcast_sender,
            config: Arc::new(config),
            bus_metrics: Arc::new(BusMetrics::new()),
        }
    }

    /// Returns a reference to the live bus metrics.
    ///
    /// Counters are updated atomically and can be read without locking.
    /// For a serializable snapshot, call [`BusMetrics::snapshot()`].
    pub fn metrics(&self) -> &BusMetrics {
        &self.bus_metrics
    }

    /// Returns the bus configuration.
    pub fn config(&self) -> &BusConfig {
        &self.config
    }

    /// Subscribe to the raw global broadcast channel.
    ///
    /// Returns a `broadcast::Receiver` for low-level consumers (e.g., the
    /// Socket.IO bridge) that handle their own lag/close recovery.
    ///
    /// Most callers should use [`receive_message`](Self::receive_message)
    /// instead, which integrates with the configured [`LagPolicy`] and
    /// metrics.
    pub fn subscribe_broadcast(&self) -> broadcast::Receiver<Vec<u8>> {
        self.broadcast_channel.subscribe()
    }

    /// Register a communication channel for an agent.
    pub async fn register_channel(
        &self,
        agent_metadata: &AgentMetadata,
        mode: CommunicationMode,
    ) -> Result<(), BusError> {
        let id = &agent_metadata.id;
        let mut agent_channels = self.agent_channels.write().await;
        let entry = agent_channels.entry(id.clone()).or_default();

        // Broadcast mode uses the global broadcast channel, no per-agent channel needed.
        if matches!(mode, CommunicationMode::Broadcast) {
            return Ok(());
        }

        // If the channel already exists, return directly.
        if entry.contains_key(&mode) {
            return Ok(());
        }

        // Resolve buffer size from config (per-mode override or default).
        let channel_config = self.config.resolve(&mode);
        let (sender, _) = broadcast::channel(channel_config.buffer_size());
        entry.insert(mode.clone(), sender);

        // PubSub mode requires registering subscriber mapping.
        if let CommunicationMode::PubSub(topic) = &mode {
            let mut topic_subs = self.topic_subscribers.write().await;
            topic_subs
                .entry(topic.clone())
                .or_default()
                .insert(id.clone());
        }

        Ok(())
    }

    /// Send a message through the bus.
    pub async fn send_message(
        &self,
        sender_id: &str,
        mode: CommunicationMode,
        message: &AgentMessage,
    ) -> Result<(), BusError> {
        let message_bytes =
            bincode::serialize(message).map_err(|e| BusError::Serialization(e.to_string()))?;

        let result = match mode {
            CommunicationMode::PointToPoint(receiver_id) => {
                self.send_point_to_point(sender_id, &receiver_id, message_bytes)
                    .await
            }
            CommunicationMode::Broadcast => self.send_broadcast(message_bytes).await,
            CommunicationMode::PubSub(ref topic) => {
                self.send_pubsub(topic, &mode, message_bytes).await
            }
        };

        match &result {
            Ok(()) => self.bus_metrics.record_send(),
            Err(_) => self.bus_metrics.record_send_error(),
        }

        result
    }

    /// Receive a message from the bus.
    pub async fn receive_message(
        &self,
        id: &str,
        mode: CommunicationMode,
    ) -> Result<Option<AgentMessage>, BusError> {
        let lag_policy = self.config.resolve(&mode).lag_policy().clone();

        if matches!(mode, CommunicationMode::Broadcast) {
            let mut receiver = self.broadcast_channel.subscribe();
            return self
                .recv_with_lag_handling(&mut receiver, &lag_policy)
                .await;
        }

        // Non-broadcast: look up the per-agent channel.
        let channel = {
            let agent_channels = self.agent_channels.read().await;
            let Some(channels) = agent_channels.get(id) else {
                return Ok(None);
            };
            let Some(channel) = channels.get(&mode) else {
                return Ok(None);
            };
            channel.clone()
        };

        let mut receiver = channel.subscribe();
        self.recv_with_lag_handling(&mut receiver, &lag_policy)
            .await
    }

    /// Unsubscribe an agent from a PubSub topic.
    pub async fn unsubscribe_topic(&self, id: &str, topic: &str) -> Result<(), BusError> {
        let mut topic_subs = self.topic_subscribers.write().await;
        if let Some(subscribers) = topic_subs.get_mut(topic) {
            subscribers.remove(id);
            if subscribers.is_empty() {
                topic_subs.remove(topic);
            }
        }
        Ok(())
    }

    // -----------------------------------------------------------------------
    // Private helpers
    // -----------------------------------------------------------------------

    /// Send a point-to-point message.
    async fn send_point_to_point(
        &self,
        sender_id: &str,
        receiver_id: &str,
        data: Vec<u8>,
    ) -> Result<(), BusError> {
        let agent_channels = self.agent_channels.read().await;
        let receiver_channels = agent_channels
            .get(receiver_id)
            .ok_or_else(|| BusError::AgentNotRegistered(receiver_id.to_owned()))?;

        let channel = receiver_channels
            .get(&CommunicationMode::PointToPoint(sender_id.to_owned()))
            .ok_or_else(|| {
                BusError::ChannelNotFound(format!(
                    "Receiver {} has no point-to-point channel with sender {}",
                    receiver_id, sender_id
                ))
            })?;

        channel
            .send(data)
            .map_err(|e| BusError::SendFailed(e.to_string()))?;
        Ok(())
    }

    /// Send a broadcast message.
    async fn send_broadcast(&self, data: Vec<u8>) -> Result<(), BusError> {
        self.broadcast_channel
            .send(data)
            .map_err(|e| BusError::SendFailed(e.to_string()))?;
        Ok(())
    }

    /// Send a PubSub message to all subscribers of the given topic.
    async fn send_pubsub(
        &self,
        topic: &str,
        mode: &CommunicationMode,
        data: Vec<u8>,
    ) -> Result<(), BusError> {
        let topic_subs = self.topic_subscribers.read().await;
        let subscribers = topic_subs.get(topic).ok_or_else(|| {
            BusError::ChannelNotFound(format!("No subscribers for topic: {}", topic))
        })?;
        let agent_channels = self.agent_channels.read().await;

        for sub_id in subscribers {
            let Some(channels) = agent_channels.get(sub_id) else {
                continue;
            };
            let Some(channel) = channels.get(mode) else {
                continue;
            };
            channel
                .send(data.clone())
                .map_err(|e| BusError::SendFailed(e.to_string()))?;
        }
        Ok(())
    }

    /// Receive from a broadcast receiver with lag policy handling.
    ///
    /// This is where the core bug fix lives: instead of `Err(_) => Ok(None)`,
    /// we match on `RecvError::Lagged(n)` and apply the configured policy.
    async fn recv_with_lag_handling(
        &self,
        receiver: &mut broadcast::Receiver<Vec<u8>>,
        lag_policy: &LagPolicyEnum,
    ) -> Result<Option<AgentMessage>, BusError> {
        loop {
            match receiver.recv().await {
                Ok(data) => {
                    self.bus_metrics.record_receive();
                    let message = bincode::deserialize(&data)
                        .map_err(|e| BusError::Serialization(e.to_string()))?;
                    return Ok(Some(message));
                }
                Err(broadcast::error::RecvError::Lagged(n)) => {
                    // Always record the lag in metrics — this is observable.
                    self.bus_metrics.record_lag(n);

                    match lag_policy {
                        LagPolicyEnum::Error => {
                            return Err(BusError::MessageLag(n));
                        }
                        LagPolicyEnum::SkipAndContinue => {
                            warn!(
                                missed = n,
                                "Bus receiver lagged, skipping {} message(s) and continuing", n,
                            );
                            // Loop back to recv() — the receiver has been
                            // advanced past the gap and will return the next
                            // available message.
                            continue;
                        }
                        // Future-proof: unknown policies treated as errors.
                        _ => return Err(BusError::MessageLag(n)),
                    }
                }
                Err(broadcast::error::RecvError::Closed) => {
                    return Ok(None);
                }
            }
        }
    }
}

impl Default for AgentBus {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::{AgentCapabilities, AgentState};
    use tokio::time::{Duration, sleep, timeout};

    fn test_agent_metadata(id: &str) -> AgentMetadata {
        AgentMetadata {
            id: id.to_string(),
            name: id.to_string(),
            description: None,
            version: None,
            capabilities: AgentCapabilities::default(),
            state: AgentState::Ready,
        }
    }

    // -----------------------------------------------------------------------
    // Original tests (backward compatibility)
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn receive_message_point_to_point_does_not_block_register_channel() {
        let bus = AgentBus::new();

        let receiver = test_agent_metadata("receiver");
        bus.register_channel(
            &receiver,
            CommunicationMode::PointToPoint("sender".to_string()),
        )
        .await
        .unwrap();

        let bus_for_receive = bus.clone();
        let receive_task = tokio::spawn(async move {
            bus_for_receive
                .receive_message(
                    "receiver",
                    CommunicationMode::PointToPoint("sender".to_string()),
                )
                .await
        });

        // Give receive_message time to subscribe and park on recv().
        sleep(Duration::from_millis(50)).await;

        let writer_meta = test_agent_metadata("writer");
        let register_res = timeout(
            Duration::from_millis(300),
            bus.register_channel(
                &writer_meta,
                CommunicationMode::PointToPoint("sender".to_string()),
            ),
        )
        .await;
        assert!(
            register_res.is_ok(),
            "register_channel should not be blocked by receive_message"
        );
        register_res.unwrap().unwrap();

        bus.send_message(
            "sender",
            CommunicationMode::PointToPoint("receiver".to_string()),
            &AgentMessage::TaskRequest {
                task_id: "task-1".to_string(),
                content: "payload".to_string(),
            },
        )
        .await
        .unwrap();

        let received = timeout(Duration::from_secs(1), receive_task)
            .await
            .expect("receive task timed out")
            .expect("receive task join failed")
            .expect("receive_message returned error");
        assert!(
            received.is_some(),
            "expected one received point-to-point message"
        );
    }

    #[tokio::test]
    async fn receive_message_broadcast_does_not_block_register_channel() {
        let bus = AgentBus::new();

        let bus_for_receive = bus.clone();
        let receive_task = tokio::spawn(async move {
            bus_for_receive
                .receive_message("receiver", CommunicationMode::Broadcast)
                .await
        });

        // Give receive_message time to subscribe and park on recv().
        sleep(Duration::from_millis(50)).await;

        let writer_meta = test_agent_metadata("writer");
        let register_res = timeout(
            Duration::from_millis(300),
            bus.register_channel(
                &writer_meta,
                CommunicationMode::PointToPoint("sender".to_string()),
            ),
        )
        .await;
        assert!(
            register_res.is_ok(),
            "register_channel should not be blocked by broadcast receive_message"
        );
        register_res.unwrap().unwrap();

        bus.send_message(
            "sender",
            CommunicationMode::Broadcast,
            &AgentMessage::TaskRequest {
                task_id: "task-2".to_string(),
                content: "payload".to_string(),
            },
        )
        .await
        .unwrap();

        let received = timeout(Duration::from_secs(1), receive_task)
            .await
            .expect("receive task timed out")
            .expect("receive task join failed")
            .expect("receive_message returned error");
        assert!(
            received.is_some(),
            "expected one received broadcast message"
        );
    }

    // -----------------------------------------------------------------------
    // New tests: backpressure and observability
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn test_default_backward_compatible() {
        // AgentBus::new() should work identically to the old hardcoded behavior.
        let bus = AgentBus::new();
        assert_eq!(bus.config().broadcast().buffer_size(), 256);
        assert_eq!(bus.metrics().messages_sent(), 0);
    }

    #[tokio::test]
    async fn test_configurable_buffer_size() {
        // Buffer size should be respected from config.
        let config = BusConfig::new(ChannelConfig::new(512));
        let bus = AgentBus::with_config(config);
        assert_eq!(bus.config().default_channel().buffer_size(), 512);
    }

    #[tokio::test]
    async fn test_metrics_tracking() {
        let config =
            BusConfig::new(ChannelConfig::new(256).with_lag_policy(LagPolicy::SkipAndContinue));
        let bus = AgentBus::with_config(config);

        // Broadcast requires at least one subscriber to accept sends.
        let _rx = bus.broadcast_channel.subscribe();

        // Send a broadcast message.
        bus.send_message(
            "sender",
            CommunicationMode::Broadcast,
            &AgentMessage::TaskRequest {
                task_id: "t1".into(),
                content: "hello".into(),
            },
        )
        .await
        .unwrap();

        assert_eq!(bus.metrics().messages_sent(), 1);
        assert_eq!(bus.metrics().messages_received(), 0);
    }

    #[tokio::test]
    async fn test_lag_error_surfaced() {
        // Small buffer + strict lag policy → BusError::MessageLag.
        let config = BusConfig::new(ChannelConfig::new(2).with_lag_policy(LagPolicy::Error));
        let bus = AgentBus::with_config(config);

        // Subscribe BEFORE sending so the receiver is listening.
        let mut receiver = bus.broadcast_channel.subscribe();

        // Send 4 messages into a 2-slot buffer → receiver will lag.
        for i in 0..4 {
            bus.broadcast_channel
                .send(
                    bincode::serialize(&AgentMessage::TaskRequest {
                        task_id: format!("t{}", i),
                        content: "x".into(),
                    })
                    .unwrap(),
                )
                .unwrap();
        }

        // The first recv should return Lagged(2) since 2 messages were overwritten.
        let lag_policy = LagPolicy::Error;
        let result = bus.recv_with_lag_handling(&mut receiver, &lag_policy).await;

        match result {
            Err(BusError::MessageLag(n)) => {
                assert!(n > 0, "should have lagged by at least 1 message");
                assert!(bus.metrics().lag_events() >= 1);
                assert!(bus.metrics().messages_dropped() >= n);
            }
            other => panic!("Expected MessageLag error, got: {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_lag_skip_policy() {
        // Small buffer + skip policy → should auto-recover and return next message.
        let config =
            BusConfig::new(ChannelConfig::new(2).with_lag_policy(LagPolicy::SkipAndContinue));
        let bus = AgentBus::with_config(config);

        let mut receiver = bus.broadcast_channel.subscribe();

        // Send 4 messages into 2-slot buffer.
        for i in 0..4 {
            bus.broadcast_channel
                .send(
                    bincode::serialize(&AgentMessage::TaskRequest {
                        task_id: format!("msg-{}", i),
                        content: format!("payload-{}", i),
                    })
                    .unwrap(),
                )
                .unwrap();
        }

        // With SkipAndContinue, recv should skip the lag and return a valid message.
        let lag_policy = LagPolicy::SkipAndContinue;
        let result = bus.recv_with_lag_handling(&mut receiver, &lag_policy).await;

        match result {
            Ok(Some(AgentMessage::TaskRequest { task_id, .. })) => {
                // Should get one of the surviving messages (msg-2 or msg-3).
                assert!(
                    task_id.starts_with("msg-"),
                    "expected a valid task_id, got: {}",
                    task_id
                );
            }
            other => panic!("Expected Ok(Some(TaskRequest)), got: {:?}", other),
        }

        // Metrics should show the lag was detected.
        assert!(bus.metrics().lag_events() >= 1);
        assert!(bus.metrics().messages_dropped() > 0);
        // And the successful receive was also recorded.
        assert_eq!(bus.metrics().messages_received(), 1);
    }

    #[tokio::test]
    async fn test_per_channel_config() {
        let config = BusConfig::default().with_override(
            CommunicationMode::PubSub("telemetry".into()),
            ChannelConfig::new(4096).with_lag_policy(LagPolicy::SkipAndContinue),
        );
        let bus = AgentBus::with_config(config);

        // Default channels should have the default buffer size.
        let default = bus
            .config()
            .resolve(&CommunicationMode::PointToPoint("x".into()));
        assert_eq!(default.buffer_size(), 256);

        // The telemetry channel should have the override.
        let telemetry = bus
            .config()
            .resolve(&CommunicationMode::PubSub("telemetry".into()));
        assert_eq!(telemetry.buffer_size(), 4096);
        assert_eq!(*telemetry.lag_policy(), LagPolicy::SkipAndContinue);
    }

    #[tokio::test]
    async fn test_metrics_snapshot_serialization() {
        let bus = AgentBus::new();

        // Broadcast requires at least one subscriber.
        let _rx = bus.broadcast_channel.subscribe();

        bus.send_message(
            "s",
            CommunicationMode::Broadcast,
            &AgentMessage::TaskRequest {
                task_id: "t".into(),
                content: "c".into(),
            },
        )
        .await
        .unwrap();

        let snap = bus.metrics().snapshot();
        let json = serde_json::to_string(&snap).unwrap();
        let de: MetricsSnapshot = serde_json::from_str(&json).unwrap();
        assert_eq!(de.messages_sent, 1);
    }

    #[tokio::test]
    async fn test_send_error_tracked() {
        // Sending to a non-existent P2P channel should increment send_errors.
        let bus = AgentBus::new();
        let result = bus
            .send_message(
                "sender",
                CommunicationMode::PointToPoint("nobody".into()),
                &AgentMessage::TaskRequest {
                    task_id: "t".into(),
                    content: "c".into(),
                },
            )
            .await;

        assert!(result.is_err());
        assert_eq!(bus.metrics().send_errors(), 1);
        assert_eq!(bus.metrics().messages_sent(), 0);
    }
}
