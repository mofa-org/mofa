use crate::agent::AgentMetadata;
use crate::message::AgentMessage;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tokio::sync::{RwLock, broadcast};

/// 通信模式枚举
/// Communication mode enumeration
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum CommunicationMode {
    /// 点对点通信（单发送方 -> 单接收方）
    /// Point-to-point communication (Single sender -> Single receiver)
    PointToPoint(String),
    /// 广播通信（单发送方 -> 所有智能体）
    /// Broadcast communication (Single sender -> All agents)
    Broadcast,
    /// 订阅-发布通信（基于主题）
    /// Pub-Sub communication (Topic-based)
    PubSub(String),
}
pub type AgentChannelMap =
    Arc<RwLock<HashMap<String, HashMap<CommunicationMode, broadcast::Sender<Vec<u8>>>>>>;
/// 通信总线核心结构体
/// Core structure for the communication bus
#[derive(Clone)]
pub struct AgentBus {
    /// 智能体-通信通道映射
    /// Agent-to-communication channel mapping
    agent_channels: AgentChannelMap,
    /// 主题-订阅者映射（PubSub 模式专用）
    /// Topic-to-subscriber mapping (Exclusive to PubSub mode)
    topic_subscribers: Arc<RwLock<HashMap<String, HashSet<String>>>>,
    /// 广播通道
    /// Broadcast channel
    broadcast_channel: broadcast::Sender<Vec<u8>>,
}

impl AgentBus {
    /// 创建通信总线实例
    /// Create a communication bus instance
    pub async fn new() -> anyhow::Result<Self> {
        let (broadcast_sender, _) = broadcast::channel(100);
        Ok(Self {
            agent_channels: Arc::new(RwLock::new(HashMap::new())),
            topic_subscribers: Arc::new(RwLock::new(HashMap::new())),
            broadcast_channel: broadcast_sender,
        })
    }

    /// 为智能体注册通信通道
    /// Register a communication channel for an agent
    pub async fn register_channel(
        &self,
        agent_metadata: &AgentMetadata,
        mode: CommunicationMode,
    ) -> anyhow::Result<()> {
        let id = &agent_metadata.id;
        let mut agent_channels = self.agent_channels.write().await;
        let entry = agent_channels.entry(id.clone()).or_default();

        // 如果是广播模式，不需要单独注册，使用全局广播通道
        // If broadcast mode, no individual registration needed; use global channel
        if matches!(mode, CommunicationMode::Broadcast) {
            return Ok(());
        }

        // 如果通道已存在，直接返回
        // If the channel already exists, return directly
        if entry.contains_key(&mode) {
            return Ok(());
        }

        // 创建新的广播通道
        // Create a new broadcast channel
        let (sender, _) = broadcast::channel(100);
        entry.insert(mode.clone(), sender);

        // PubSub 模式需注册订阅者映射
        // PubSub mode requires registering subscriber mapping
        if let CommunicationMode::PubSub(topic) = &mode {
            let mut topic_subs = self.topic_subscribers.write().await;
            topic_subs
                .entry(topic.clone())
                .or_default()
                .insert(id.clone());
        }

        Ok(())
    }

    // 核心：完善点对点消息发送逻辑
    // Core: Refine the point-to-point message sending logic
    pub async fn send_message(
        &self,
        sender_id: &str,
        mode: CommunicationMode,
        message: &AgentMessage,
    ) -> anyhow::Result<()> {
        let message_bytes = bincode::serialize(message)?;

        match mode {
            // 点对点模式：根据接收方 ID 查找通道并发送
            // Point-to-point mode: Find channel by receiver ID and send
            CommunicationMode::PointToPoint(receiver_id) => {
                let agent_channels = self.agent_channels.read().await;
                // 1. 校验接收方是否存在并注册了对应通道
                // 1. Verify if receiver exists and has registered the channel
                let Some(receiver_channels) = agent_channels.get(&receiver_id) else {
                    return Err(anyhow::anyhow!("Receiver agent {} not found", receiver_id));
                };
                let Some(channel) =
                    receiver_channels.get(&CommunicationMode::PointToPoint(sender_id.to_string()))
                else {
                    return Err(anyhow::anyhow!(
                        "Receiver {} has no point-to-point channel with sender {}",
                        receiver_id,
                        sender_id
                    ));
                };
                // 2. 发送消息
                // 2. Send the message
                channel.send(message_bytes)?;
            }
            CommunicationMode::Broadcast => {
                // 使用全局广播通道
                // Use the global broadcast channel
                self.broadcast_channel.send(message_bytes)?;
            }
            CommunicationMode::PubSub(ref topic) => {
                let topic_subs = self.topic_subscribers.read().await;
                let subscribers = topic_subs
                    .get(topic)
                    .ok_or_else(|| anyhow::anyhow!("No subscribers for topic: {}", topic))?;
                let agent_channels = self.agent_channels.read().await;

                for sub_id in subscribers {
                    let Some(channels) = agent_channels.get(sub_id) else {
                        continue;
                    };
                    let Some(channel) = channels.get(&mode) else {
                        continue;
                    };
                    channel.send(message_bytes.clone())?;
                }
            }
        }

        Ok(())
    }

    pub async fn receive_message(
        &self,
        id: &str,
        mode: CommunicationMode,
    ) -> anyhow::Result<Option<AgentMessage>> {
        let agent_channels = self.agent_channels.read().await;

        // 处理广播模式
        // Handle broadcast mode
        if matches!(mode, CommunicationMode::Broadcast) {
            let mut receiver = self.broadcast_channel.subscribe();
            match receiver.recv().await {
                Ok(data) => {
                    let message = bincode::deserialize(&data)?;
                    Ok(Some(message))
                }
                Err(_) => Ok(None),
            }
        } else {
            // 处理其他模式
            // Handle other modes
            let Some(channels) = agent_channels.get(id) else {
                return Ok(None);
            };
            let Some(channel) = channels.get(&mode) else {
                return Ok(None);
            };

            let mut receiver = channel.subscribe();
            match receiver.recv().await {
                Ok(data) => {
                    let message = bincode::deserialize(&data)?;
                    Ok(Some(message))
                }
                Err(_) => Ok(None),
            }
        }
    }

    pub async fn unsubscribe_topic(&self, id: &str, topic: &str) -> anyhow::Result<()> {
        let mut topic_subs = self.topic_subscribers.write().await;
        if let Some(subscribers) = topic_subs.get_mut(topic) {
            subscribers.remove(id);
            if subscribers.is_empty() {
                topic_subs.remove(topic);
            }
        }
        Ok(())
    }
}
