//! DoraChannel 封装
//! DoraChannel Wrapper
//!
//! 提供与 dora-rs 集成的跨智能体通信通道
//! Provides cross-agent communication channels integrated with dora-rs

use crate::dora_adapter::error::{DoraError, DoraResult};
use ::tracing::{debug, info};
use mofa_kernel::message::AgentMessage;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{Mutex, RwLock, broadcast, mpsc};
use tokio::time::timeout;

/// Type alias for the receiver storage type to reduce complexity
type ReceiverMap = Arc<RwLock<HashMap<String, Arc<Mutex<mpsc::Receiver<MessageEnvelope>>>>>>;

/// 通道配置
/// Channel configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelConfig {
    /// 通道 ID
    /// Channel ID
    pub channel_id: String,
    /// 缓冲区大小
    /// Buffer size
    pub buffer_size: usize,
    /// 消息超时时间
    /// Message timeout duration
    pub message_timeout: Duration,
    /// 是否启用持久化
    /// Whether persistence is enabled
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

/// 消息信封 - 包含元数据的消息包装
/// Message Envelope - Message wrapper containing metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageEnvelope {
    /// 消息 ID
    /// Message ID
    pub message_id: String,
    /// 发送方 ID
    /// Sender ID
    pub sender_id: String,
    /// 接收方 ID（None 表示广播）
    /// Receiver ID (None means broadcast)
    pub receiver_id: Option<String>,
    /// 主题（用于 PubSub）
    /// Topic (for PubSub)
    pub topic: Option<String>,
    /// 时间戳
    /// Timestamp
    pub timestamp: u64,
    /// 消息内容
    /// Message payload
    pub payload: Vec<u8>,
    /// 元数据
    /// Metadata
    pub metadata: HashMap<String, String>,
}

impl MessageEnvelope {
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

    /// 设置接收方（点对点）
    /// Set receiver (point-to-point)
    pub fn to(mut self, receiver_id: &str) -> Self {
        self.receiver_id = Some(receiver_id.to_string());
        self
    }

    /// 设置主题（PubSub）
    /// Set topic (PubSub)
    pub fn with_topic(mut self, topic: &str) -> Self {
        self.topic = Some(topic.to_string());
        self
    }

    /// 添加元数据
    /// Add metadata
    pub fn with_metadata(mut self, key: &str, value: &str) -> Self {
        self.metadata.insert(key.to_string(), value.to_string());
        self
    }

    /// 从 AgentMessage 创建
    /// Create from AgentMessage
    pub fn from_agent_message(sender_id: &str, message: &AgentMessage) -> DoraResult<Self> {
        let payload = bincode::serialize(message)?;
        Ok(Self::new(sender_id, payload))
    }

    /// 解析为 AgentMessage
    /// Parse as AgentMessage
    pub fn to_agent_message(&self) -> DoraResult<AgentMessage> {
        bincode::deserialize(&self.payload)
            .map_err(|e| DoraError::DeserializationError(e.to_string()))
    }
}

/// dora-rs 集成通道
/// dora-rs integrated channel
pub struct DoraChannel {
    config: ChannelConfig,
    /// 点对点通道：接收方ID -> 发送器
    /// P2P channels: Receiver ID -> Sender
    p2p_channels: Arc<RwLock<HashMap<String, mpsc::Sender<MessageEnvelope>>>>,
    /// 广播通道
    /// Broadcast channel
    broadcast_tx: broadcast::Sender<MessageEnvelope>,
    /// 主题订阅：主题 -> 订阅者ID列表
    /// Topic subscriptions: Topic -> Subscriber ID list
    topic_subscribers: Arc<RwLock<HashMap<String, Vec<String>>>>,
    /// 主题通道：主题 -> 发送器
    /// Topic channels: Topic -> Sender
    topic_channels: Arc<RwLock<HashMap<String, broadcast::Sender<MessageEnvelope>>>>,
    /// 接收器存储：智能体ID -> 接收器
    /// Receiver storage: Agent ID -> Receiver
    receivers: ReceiverMap,
}

impl DoraChannel {
    /// 创建新通道
    /// Create a new channel
    pub fn new(config: ChannelConfig) -> Self {
        let (broadcast_tx, _) = broadcast::channel(config.buffer_size);
        Self {
            config,
            p2p_channels: Arc::new(RwLock::new(HashMap::new())),
            broadcast_tx,
            topic_subscribers: Arc::new(RwLock::new(HashMap::new())),
            topic_channels: Arc::new(RwLock::new(HashMap::new())),
            receivers: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// 获取配置
    /// Get configuration
    pub fn config(&self) -> &ChannelConfig {
        &self.config
    }

    /// 注册智能体
    /// Register agent
    pub async fn register_agent(&self, agent_id: &str) -> DoraResult<()> {
        let (tx, rx) = mpsc::channel(self.config.buffer_size);

        let mut p2p_channels = self.p2p_channels.write().await;
        p2p_channels.insert(agent_id.to_string(), tx);

        let mut receivers = self.receivers.write().await;
        receivers.insert(agent_id.to_string(), Arc::new(Mutex::new(rx)));

        info!(
            "Agent {} registered to channel {}",
            agent_id, self.config.channel_id
        );
        Ok(())
    }

    /// 注销智能体
    /// Unregister agent
    pub async fn unregister_agent(&self, agent_id: &str) -> DoraResult<()> {
        let mut p2p_channels = self.p2p_channels.write().await;
        p2p_channels.remove(agent_id);

        let mut receivers = self.receivers.write().await;
        receivers.remove(agent_id);

        // 从所有主题中移除
        // Remove from all topics
        let mut topic_subs = self.topic_subscribers.write().await;
        for subscribers in topic_subs.values_mut() {
            subscribers.retain(|id| id != agent_id);
        }

        info!(
            "Agent {} unregistered from channel {}",
            agent_id, self.config.channel_id
        );
        Ok(())
    }

    /// 订阅主题
    /// Subscribe to topic
    pub async fn subscribe(&self, agent_id: &str, topic: &str) -> DoraResult<()> {
        let mut topic_subs = self.topic_subscribers.write().await;
        topic_subs
            .entry(topic.to_string())
            .or_default()
            .push(agent_id.to_string());

        // 确保主题通道存在
        // Ensure topic channel exists
        let mut topic_channels = self.topic_channels.write().await;
        if !topic_channels.contains_key(topic) {
            let (tx, _) = broadcast::channel(self.config.buffer_size);
            topic_channels.insert(topic.to_string(), tx);
        }

        debug!("Agent {} subscribed to topic {}", agent_id, topic);
        Ok(())
    }

    /// 取消订阅主题
    /// Unsubscribe from topic
    pub async fn unsubscribe(&self, agent_id: &str, topic: &str) -> DoraResult<()> {
        let mut topic_subs = self.topic_subscribers.write().await;
        if let Some(subscribers) = topic_subs.get_mut(topic) {
            subscribers.retain(|id| id != agent_id);
            if subscribers.is_empty() {
                topic_subs.remove(topic);
            }
        }
        debug!("Agent {} unsubscribed from topic {}", agent_id, topic);
        Ok(())
    }

    /// 发送点对点消息
    /// Send point-to-point message
    pub async fn send_p2p(&self, envelope: MessageEnvelope) -> DoraResult<()> {
        let receiver_id = envelope
            .receiver_id
            .clone()
            .ok_or_else(|| DoraError::ChannelError("No receiver specified for P2P".to_string()))?;

        let p2p_channels = self.p2p_channels.read().await;
        let tx = p2p_channels.get(&receiver_id).ok_or_else(|| {
            DoraError::AgentNotFound(format!("Receiver {} not registered", receiver_id))
        })?;

        tx.send(envelope)
            .await
            .map_err(|e| DoraError::ChannelError(e.to_string()))?;

        debug!("P2P message sent to {}", receiver_id);
        Ok(())
    }

    /// 广播消息
    /// Broadcast message
    pub async fn broadcast(&self, envelope: MessageEnvelope) -> DoraResult<()> {
        // 如果没有接收者，send 会返回错误，但这不应该是致命错误
        // If there are no receivers, send returns an error, but this shouldn't be fatal
        match self.broadcast_tx.send(envelope) {
            Ok(receiver_count) => {
                debug!("Broadcast message sent to {} receivers", receiver_count);
            }
            Err(_) => {
                debug!("Broadcast message sent but no receivers");
            }
        }
        Ok(())
    }

    /// 发布到主题
    /// Publish to topic
    pub async fn publish(&self, envelope: MessageEnvelope) -> DoraResult<()> {
        let topic = envelope
            .topic
            .clone()
            .ok_or_else(|| DoraError::ChannelError("No topic specified".to_string()))?;

        let topic_channels = self.topic_channels.read().await;
        let tx = topic_channels
            .get(&topic)
            .ok_or_else(|| DoraError::ChannelError(format!("Topic {} not found", topic)))?;

        // 如果没有接收者，send 会返回错误，但这不应该是致命错误
        // If there are no receivers, send returns an error, but this shouldn't be fatal
        match tx.send(envelope) {
            Ok(receiver_count) => {
                debug!(
                    "Message published to topic {} with {} receivers",
                    topic, receiver_count
                );
            }
            Err(_) => {
                debug!("Message published to topic {} but no receivers", topic);
            }
        }
        Ok(())
    }

    /// 接收点对点消息（阻塞）
    /// Receive point-to-point message (blocking)
    pub async fn receive_p2p(&self, agent_id: &str) -> DoraResult<Option<MessageEnvelope>> {
        let rx = {
            let receivers = self.receivers.read().await;
            receivers.get(agent_id).cloned().ok_or_else(|| {
                DoraError::AgentNotFound(format!("Agent {} not registered", agent_id))
            })?
        };

        let mut rx_guard = rx.lock().await;
        match timeout(self.config.message_timeout, rx_guard.recv()).await {
            Ok(Some(envelope)) => Ok(Some(envelope)),
            Ok(None) => Ok(None),
            Err(_) => Err(DoraError::Timeout("Receive timeout".to_string())),
        }
    }

    /// 尝试接收点对点消息（非阻塞）
    /// Try to receive point-to-point message (non-blocking)
    pub async fn try_receive_p2p(&self, agent_id: &str) -> DoraResult<Option<MessageEnvelope>> {
        let rx = {
            let receivers = self.receivers.read().await;
            receivers.get(agent_id).cloned().ok_or_else(|| {
                DoraError::AgentNotFound(format!("Agent {} not registered", agent_id))
            })?
        };

        let mut rx_guard = rx.lock().await;
        match rx_guard.try_recv() {
            Ok(envelope) => Ok(Some(envelope)),
            Err(mpsc::error::TryRecvError::Empty) => Ok(None),
            Err(mpsc::error::TryRecvError::Disconnected) => {
                Err(DoraError::ChannelError("Channel disconnected".to_string()))
            }
        }
    }

    /// 订阅广播（返回接收器）
    /// Subscribe to broadcast (returns receiver)
    pub fn subscribe_broadcast(&self) -> broadcast::Receiver<MessageEnvelope> {
        self.broadcast_tx.subscribe()
    }

    /// 订阅主题（返回接收器）
    /// Subscribe to topic (returns receiver)
    pub async fn subscribe_topic(
        &self,
        topic: &str,
    ) -> DoraResult<broadcast::Receiver<MessageEnvelope>> {
        let topic_channels = self.topic_channels.read().await;
        let tx = topic_channels
            .get(topic)
            .ok_or_else(|| DoraError::ChannelError(format!("Topic {} not found", topic)))?;
        Ok(tx.subscribe())
    }

    /// 获取主题订阅者列表
    /// Get topic subscriber list
    pub async fn get_topic_subscribers(&self, topic: &str) -> Vec<String> {
        let topic_subs = self.topic_subscribers.read().await;
        topic_subs.get(topic).cloned().unwrap_or_default()
    }

    /// 获取所有已注册的智能体
    /// Get all registered agents
    pub async fn registered_agents(&self) -> Vec<String> {
        let p2p_channels = self.p2p_channels.read().await;
        p2p_channels.keys().cloned().collect()
    }
}

/// 通道管理器 - 管理多个通道
/// Channel Manager - Manages multiple channels
pub struct ChannelManager {
    channels: Arc<RwLock<HashMap<String, Arc<DoraChannel>>>>,
    default_config: ChannelConfig,
}

impl ChannelManager {
    pub fn new() -> Self {
        Self {
            channels: Arc::new(RwLock::new(HashMap::new())),
            default_config: ChannelConfig::default(),
        }
    }

    /// 创建或获取通道
    /// Create or retrieve a channel
    pub async fn get_or_create_channel(&self, channel_id: &str) -> Arc<DoraChannel> {
        let channels = self.channels.read().await;
        if let Some(channel) = channels.get(channel_id) {
            return channel.clone();
        }
        drop(channels);

        let config = ChannelConfig {
            channel_id: channel_id.to_string(),
            ..self.default_config.clone()
        };
        let channel = Arc::new(DoraChannel::new(config));

        let mut channels = self.channels.write().await;
        channels.insert(channel_id.to_string(), channel.clone());
        channel
    }

    /// 删除通道
    /// Remove a channel
    pub async fn remove_channel(&self, channel_id: &str) -> Option<Arc<DoraChannel>> {
        let mut channels = self.channels.write().await;
        channels.remove(channel_id)
    }

    /// 获取所有通道 ID
    /// Get all channel IDs
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
    use super::{ChannelConfig, ChannelManager, DoraChannel, MessageEnvelope};
    use std::sync::Arc;
    use std::time::Duration;

    #[tokio::test]
    async fn test_p2p_communication() {
        let channel = DoraChannel::new(ChannelConfig::default());

        // 注册两个智能体
        // Register two agents
        channel.register_agent("agent1").await.unwrap();
        channel.register_agent("agent2").await.unwrap();

        // agent1 发送消息给 agent2
        // agent1 sends message to agent2
        let envelope = MessageEnvelope::new("agent1", b"Hello agent2".to_vec()).to("agent2");
        channel.send_p2p(envelope).await.unwrap();

        // agent2 接收消息
        // agent2 receives message
        let received = channel.try_receive_p2p("agent2").await.unwrap();
        assert!(received.is_some());
        assert_eq!(received.unwrap().payload, b"Hello agent2");
    }

    #[tokio::test]
    async fn test_pubsub() {
        let channel = DoraChannel::new(ChannelConfig::default());

        channel.register_agent("publisher").await.unwrap();
        channel.register_agent("subscriber").await.unwrap();

        // 订阅主题
        // Subscribe to topic
        channel.subscribe("subscriber", "news").await.unwrap();

        // 获取主题接收器
        // Get topic receiver
        let mut rx = channel.subscribe_topic("news").await.unwrap();

        // 发布消息
        // Publish message
        let envelope =
            MessageEnvelope::new("publisher", b"Breaking news".to_vec()).with_topic("news");
        channel.publish(envelope).await.unwrap();

        // 接收消息
        // Receive message
        let received = rx.recv().await.unwrap();
        assert_eq!(received.payload, b"Breaking news");
    }

    #[tokio::test]
    async fn test_channel_manager() {
        let manager = ChannelManager::new();

        let channel1 = manager.get_or_create_channel("channel1").await;
        let channel2 = manager.get_or_create_channel("channel1").await;

        // 应该返回同一个通道
        // Should return the same channel
        assert_eq!(channel1.config().channel_id, channel2.config().channel_id);

        let ids = manager.channel_ids().await;
        assert_eq!(ids.len(), 1);
        assert!(ids.contains(&"channel1".to_string()));
    }

    #[tokio::test]
    async fn test_p2p_receive_deadlock() {
        let channel = Arc::new(DoraChannel::new(ChannelConfig {
            message_timeout: Duration::from_millis(500),
            ..ChannelConfig::default()
        }));
        channel.register_agent("reader").await.unwrap();
        channel.register_agent("writer").await.unwrap();

        let channel_clone = channel.clone();

        // start a receive on reader
        let reader_task = tokio::spawn(async move {
            let _ = channel_clone.receive_p2p("reader").await;
        });
        tokio::time::sleep(Duration::from_millis(50)).await;

        let start_time = std::time::Instant::now();

        // registering a new agent.
        channel.register_agent("new_agent").await.unwrap();
        let elapsed = start_time.elapsed();
        reader_task.await.unwrap();

        // the bug exists if register_agent was blocked
        assert!(
            elapsed < Duration::from_millis(400),
            "Deadlock reproduced: register_agent was blocked for {:?} by receive_p2p!",
            elapsed
        );
    }
}
