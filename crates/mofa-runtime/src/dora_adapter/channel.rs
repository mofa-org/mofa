//! DoraChannel 封装
//!
//! 提供与 dora-rs 集成的跨智能体通信通道

use crate::dora_adapter::error::{DoraError, DoraResult};
use ::tracing::{debug, info};
use mofa_kernel::message::AgentMessage;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{RwLock, broadcast, mpsc};
use tokio::time::timeout;

/// 通道配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelConfig {
    /// 通道 ID
    pub channel_id: String,
    /// 缓冲区大小
    pub buffer_size: usize,
    /// 消息超时时间
    pub message_timeout: Duration,
    /// 是否启用持久化
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
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageEnvelope {
    /// 消息 ID
    pub message_id: String,
    /// 发送方 ID
    pub sender_id: String,
    /// 接收方 ID（None 表示广播）
    pub receiver_id: Option<String>,
    /// 主题（用于 PubSub）
    pub topic: Option<String>,
    /// 时间戳
    pub timestamp: u64,
    /// 消息内容
    pub payload: Vec<u8>,
    /// 元数据
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
    pub fn to(mut self, receiver_id: &str) -> Self {
        self.receiver_id = Some(receiver_id.to_string());
        self
    }

    /// 设置主题（PubSub）
    pub fn with_topic(mut self, topic: &str) -> Self {
        self.topic = Some(topic.to_string());
        self
    }

    /// 添加元数据
    pub fn with_metadata(mut self, key: &str, value: &str) -> Self {
        self.metadata.insert(key.to_string(), value.to_string());
        self
    }

    /// 从 AgentMessage 创建
    pub fn from_agent_message(sender_id: &str, message: &AgentMessage) -> DoraResult<Self> {
        let payload = bincode::serialize(message)?;
        Ok(Self::new(sender_id, payload))
    }

    /// 解析为 AgentMessage
    pub fn to_agent_message(&self) -> DoraResult<AgentMessage> {
        bincode::deserialize(&self.payload)
            .map_err(|e| DoraError::DeserializationError(e.to_string()))
    }
}

/// dora-rs 集成通道
pub struct DoraChannel {
    config: ChannelConfig,
    /// 点对点通道：接收方ID -> 发送器
    p2p_channels: Arc<RwLock<HashMap<String, mpsc::Sender<MessageEnvelope>>>>,
    /// 广播通道
    broadcast_tx: broadcast::Sender<MessageEnvelope>,
    /// 主题订阅：主题 -> 订阅者ID列表
    topic_subscribers: Arc<RwLock<HashMap<String, Vec<String>>>>,
    /// 主题通道：主题 -> 发送器
    topic_channels: Arc<RwLock<HashMap<String, broadcast::Sender<MessageEnvelope>>>>,
    /// 接收器存储：智能体ID -> 接收器
    receivers: Arc<RwLock<HashMap<String, mpsc::Receiver<MessageEnvelope>>>>,
}

impl DoraChannel {
    /// 创建新通道
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
    pub fn config(&self) -> &ChannelConfig {
        &self.config
    }

    /// 注册智能体
    pub async fn register_agent(&self, agent_id: &str) -> DoraResult<()> {
        let (tx, rx) = mpsc::channel(self.config.buffer_size);

        let mut p2p_channels = self.p2p_channels.write().await;
        p2p_channels.insert(agent_id.to_string(), tx);

        let mut receivers = self.receivers.write().await;
        receivers.insert(agent_id.to_string(), rx);

        info!(
            "Agent {} registered to channel {}",
            agent_id, self.config.channel_id
        );
        Ok(())
    }

    /// 注销智能体
    pub async fn unregister_agent(&self, agent_id: &str) -> DoraResult<()> {
        let mut p2p_channels = self.p2p_channels.write().await;
        p2p_channels.remove(agent_id);

        let mut receivers = self.receivers.write().await;
        receivers.remove(agent_id);

        // 从所有主题中移除
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
    pub async fn subscribe(&self, agent_id: &str, topic: &str) -> DoraResult<()> {
        let mut topic_subs = self.topic_subscribers.write().await;
        topic_subs
            .entry(topic.to_string())
            .or_default()
            .push(agent_id.to_string());

        // 确保主题通道存在
        let mut topic_channels = self.topic_channels.write().await;
        if !topic_channels.contains_key(topic) {
            let (tx, _) = broadcast::channel(self.config.buffer_size);
            topic_channels.insert(topic.to_string(), tx);
        }

        debug!("Agent {} subscribed to topic {}", agent_id, topic);
        Ok(())
    }

    /// 取消订阅主题
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
    pub async fn broadcast(&self, envelope: MessageEnvelope) -> DoraResult<()> {
        // 如果没有接收者，send 会返回错误，但这不应该是致命错误
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
    pub async fn receive_p2p(&self, agent_id: &str) -> DoraResult<Option<MessageEnvelope>> {
        let mut receivers = self.receivers.write().await;
        let rx = receivers.get_mut(agent_id).ok_or_else(|| {
            DoraError::AgentNotFound(format!("Agent {} not registered", agent_id))
        })?;

        match timeout(self.config.message_timeout, rx.recv()).await {
            Ok(Some(envelope)) => Ok(Some(envelope)),
            Ok(None) => Ok(None),
            Err(_) => Err(DoraError::Timeout("Receive timeout".to_string())),
        }
    }

    /// 尝试接收点对点消息（非阻塞）
    pub async fn try_receive_p2p(&self, agent_id: &str) -> DoraResult<Option<MessageEnvelope>> {
        let mut receivers = self.receivers.write().await;
        let rx = receivers.get_mut(agent_id).ok_or_else(|| {
            DoraError::AgentNotFound(format!("Agent {} not registered", agent_id))
        })?;

        match rx.try_recv() {
            Ok(envelope) => Ok(Some(envelope)),
            Err(mpsc::error::TryRecvError::Empty) => Ok(None),
            Err(mpsc::error::TryRecvError::Disconnected) => {
                Err(DoraError::ChannelError("Channel disconnected".to_string()))
            }
        }
    }

    /// 订阅广播（返回接收器）
    pub fn subscribe_broadcast(&self) -> broadcast::Receiver<MessageEnvelope> {
        self.broadcast_tx.subscribe()
    }

    /// 订阅主题（返回接收器）
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
    pub async fn get_topic_subscribers(&self, topic: &str) -> Vec<String> {
        let topic_subs = self.topic_subscribers.read().await;
        topic_subs.get(topic).cloned().unwrap_or_default()
    }

    /// 获取所有已注册的智能体
    pub async fn registered_agents(&self) -> Vec<String> {
        let p2p_channels = self.p2p_channels.read().await;
        p2p_channels.keys().cloned().collect()
    }
}

/// 通道管理器 - 管理多个通道
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
    pub async fn remove_channel(&self, channel_id: &str) -> Option<Arc<DoraChannel>> {
        let mut channels = self.channels.write().await;
        channels.remove(channel_id)
    }

    /// 获取所有通道 ID
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

    #[tokio::test]
    async fn test_p2p_communication() {
        let channel = DoraChannel::new(ChannelConfig::default());

        // 注册两个智能体
        channel.register_agent("agent1").await.unwrap();
        channel.register_agent("agent2").await.unwrap();

        // agent1 发送消息给 agent2
        let envelope = MessageEnvelope::new("agent1", b"Hello agent2".to_vec()).to("agent2");
        channel.send_p2p(envelope).await.unwrap();

        // agent2 接收消息
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
        channel.subscribe("subscriber", "news").await.unwrap();

        // 获取主题接收器
        let mut rx = channel.subscribe_topic("news").await.unwrap();

        // 发布消息
        let envelope =
            MessageEnvelope::new("publisher", b"Breaking news".to_vec()).with_topic("news");
        channel.publish(envelope).await.unwrap();

        // 接收消息
        let received = rx.recv().await.unwrap();
        assert_eq!(received.payload, b"Breaking news");
    }

    #[tokio::test]
    async fn test_channel_manager() {
        let manager = ChannelManager::new();

        let channel1 = manager.get_or_create_channel("channel1").await;
        let channel2 = manager.get_or_create_channel("channel1").await;

        // 应该返回同一个通道
        assert_eq!(channel1.config().channel_id, channel2.config().channel_id);

        let ids = manager.channel_ids().await;
        assert_eq!(ids.len(), 1);
        assert!(ids.contains(&"channel1".to_string()));
    }
}
