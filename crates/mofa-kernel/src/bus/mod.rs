use crate::agent::AgentMetadata;
use crate::message::AgentMessage;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tokio::sync::RwLock;

pub mod config;
pub mod metrics;
pub mod queue;

use config::EventBusConfig;
use metrics::EventBusMetrics;
use queue::EventQueue;

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
#[allow(clippy::type_complexity)]
pub type AgentChannelMap =
    Arc<RwLock<HashMap<String, HashMap<CommunicationMode, EventQueue>>>>;

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
    /// 广播订阅者队列
    /// Broadcast subscriber queues
    broadcast_subscribers: Arc<RwLock<HashMap<String, EventQueue>>>,
    /// Bus configuration
    config: EventBusConfig,
    /// Bus metrics
    metrics: Arc<EventBusMetrics>,
}

impl AgentBus {
    /// 创建通信总线实例
    /// Create a communication bus instance
    pub async fn new() -> anyhow::Result<Self> {
        Self::new_with_config(EventBusConfig::default()).await
    }

    /// Create a communication bus instance with specific configuration
    pub async fn new_with_config(config: EventBusConfig) -> anyhow::Result<Self> {
        Ok(Self {
            agent_channels: Arc::new(RwLock::new(HashMap::new())),
            topic_subscribers: Arc::new(RwLock::new(HashMap::new())),
            broadcast_subscribers: Arc::new(RwLock::new(HashMap::new())),
            config,
            metrics: Arc::new(EventBusMetrics::new()),
        })
    }

    /// Get current metrics for monitoring
    pub fn metrics(&self) -> Arc<EventBusMetrics> {
        Arc::clone(&self.metrics)
    }

    /// Get bus configuration
    pub fn config(&self) -> &EventBusConfig {
        &self.config
    }

    /// 为智能体注册通信通道
    /// Register a communication channel for an agent
    pub async fn register_channel(
        &self,
        agent_metadata: &AgentMetadata,
        mode: CommunicationMode,
    ) -> anyhow::Result<()> {
        let id = &agent_metadata.id;

        // 如果是广播模式，注册到全局广播队列
        // If broadcast mode, register to the global broadcast queues
        if matches!(mode, CommunicationMode::Broadcast) {
            let mut subs = self.broadcast_subscribers.write().await;
            if !subs.contains_key(id) {
                let (cap, strat) = self.config.get_topic_config("broadcast");
                subs.insert(id.clone(), EventQueue::new(cap, strat, Arc::clone(&self.metrics)));
            }
            return Ok(());
        }

        let mut agent_channels = self.agent_channels.write().await;
        let entry = agent_channels.entry(id.clone()).or_default();

        // 如果通道已存在，直接返回
        // If the channel already exists, return directly
        if entry.contains_key(&mode) {
            return Ok(());
        }

        // 创建新的通信通道并应用相关的背压策略和容量限制
        // Create a new communication channel and apply backpressure strategies
        let topic_name = match &mode {
            CommunicationMode::PubSub(topic) => topic.clone(),
            _ => format!("{:?}", mode),
        };
        let (cap, strat) = self.config.get_topic_config(&topic_name);
        let queue = EventQueue::new(cap, strat, Arc::clone(&self.metrics));
        entry.insert(mode.clone(), queue);

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
        let priority = message.priority();

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
                channel.send(priority, message_bytes).await?;
            }
            CommunicationMode::Broadcast => {
                // 将消息分发给所有注册的广播接收者
                // Distribute message to all registered broadcast subscribers
                let subs = self.broadcast_subscribers.read().await;
                for (id, queue) in subs.iter() {
                    // Do not send broadcast to sender itself
                    if id != sender_id {
                        queue.send(priority.clone(), message_bytes.clone()).await?;
                    }
                }
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
                    channel.send(priority.clone(), message_bytes.clone()).await?;
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
            let subs = self.broadcast_subscribers.read().await;
            if let Some(queue) = subs.get(id) {
                // We keep a reference to a clone of the queue so we don't hold the read lock while awaiting
                let queue = queue.clone();
                drop(subs);
                drop(agent_channels);
                match queue.recv().await {
                    Ok((_, data)) => {
                        let message = bincode::deserialize(&data)?;
                        Ok(Some(message))
                    }
                    Err(_) => Ok(None),
                }
            } else {
                Ok(None)
            }
        } else {
            // 处理其他模式
            // Handle other modes
            let Some(channels) = agent_channels.get(id) else {
                return Ok(None);
            };
            let Some(queue) = channels.get(&mode) else {
                return Ok(None);
            };
            let queue = queue.clone();
            drop(agent_channels);

            match queue.recv().await {
                Ok((_, data)) => {
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

#[cfg(test)]
mod tests;
