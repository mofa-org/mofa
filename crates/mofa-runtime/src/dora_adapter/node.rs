//! DoraNode 封装
//! DoraNode Wrapper
//!
//! 封装 dora-rs 的 Node API，提供智能体生命周期管理
//! Wraps dora-rs Node API to provide agent lifecycle management

use crate::dora_adapter::error::{DoraError, DoraResult};
use crate::interrupt::AgentInterrupt;
use ::tracing::{debug, info, warn};
use dora_node_api::Event;
use mofa_kernel::message::{AgentEvent, AgentMessage, TaskPriority, TaskRequest};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{RwLock, mpsc};

/// DoraNode 配置
/// DoraNode Configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DoraNodeConfig {
    /// 节点唯一标识
    /// Unique node identifier
    pub node_id: String,
    /// 节点名称
    /// Node name
    pub name: String,
    /// 输入端口列表
    /// List of input ports
    pub inputs: Vec<String>,
    /// 输出端口列表
    /// List of output ports
    pub outputs: Vec<String>,
    /// 事件缓冲区大小
    /// Event buffer size
    pub event_buffer_size: usize,
    /// 默认超时时间
    /// Default timeout duration
    pub default_timeout: Duration,
    /// 自定义配置
    /// Custom configuration
    pub custom_config: HashMap<String, String>,
}

impl Default for DoraNodeConfig {
    fn default() -> Self {
        Self {
            node_id: uuid::Uuid::now_v7().to_string(),
            name: "default_node".to_string(),
            inputs: vec![],
            outputs: vec![],
            event_buffer_size: 1024,
            default_timeout: Duration::from_secs(30),
            custom_config: HashMap::new(),
        }
    }
}

/// 节点状态
/// Node State
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

/// 封装 dora-rs DoraNode 的智能体节点
/// Agent node wrapping dora-rs DoraNode
pub struct DoraAgentNode {
    config: DoraNodeConfig,
    state: Arc<RwLock<NodeState>>,
    interrupt: AgentInterrupt,
    /// 内部事件发送器
    /// Internal event transmitter
    event_tx: mpsc::Sender<AgentEvent>,
    /// 内部事件接收器
    /// Internal event receiver
    event_rx: Arc<RwLock<mpsc::Receiver<AgentEvent>>>,
    /// 输出通道映射
    /// Output channel mapping
    output_channels: Arc<RwLock<HashMap<String, mpsc::Sender<Vec<u8>>>>>,
}

impl DoraAgentNode {
    /// 创建新的 DoraAgentNode
    /// Create a new DoraAgentNode
    pub fn new(config: DoraNodeConfig) -> Self {
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

    /// 获取节点配置
    /// Get node configuration
    pub fn config(&self) -> &DoraNodeConfig {
        &self.config
    }

    /// 获取节点状态
    /// Get node state
    pub async fn state(&self) -> NodeState {
        self.state.read().await.clone()
    }

    /// 获取中断句柄
    /// Get interrupt handle
    pub fn interrupt(&self) -> &AgentInterrupt {
        &self.interrupt
    }

    /// 初始化节点（模拟 dora-rs 节点初始化）
    /// Initialize node (simulates dora-rs node initialization)
    pub async fn init(&self) -> DoraResult<()> {
        let mut state = self.state.write().await;
        if *state != NodeState::Created {
            return Err(DoraError::NodeInitError(
                "Node already initialized".to_string(),
            ));
        }
        *state = NodeState::Initializing;

        // 初始化输出通道
        // Initialize output channels
        let mut output_channels = self.output_channels.write().await;
        for output in &self.config.outputs {
            let (tx, _rx) = mpsc::channel(self.config.event_buffer_size);
            output_channels.insert(output.clone(), tx);
        }

        *state = NodeState::Running;
        info!("DoraAgentNode {} initialized", self.config.node_id);
        Ok(())
    }

    /// 发送消息到指定输出端口
    /// Send message to specified output port
    pub async fn send_output(&self, output_id: &str, data: Vec<u8>) -> DoraResult<()> {
        let state = self.state.read().await;
        if *state != NodeState::Running {
            return Err(DoraError::NodeNotRunning);
        }

        let output_channels = self.output_channels.read().await;
        if let Some(tx) = output_channels.get(output_id) {
            tx.send(data)
                .await
                .map_err(|e| DoraError::ChannelError(e.to_string()))?;
            debug!("Sent data to output: {}", output_id);
        } else {
            warn!("Output channel {} not found", output_id);
        }
        Ok(())
    }

    /// 发送序列化的 AgentMessage
    /// Send serialized AgentMessage
    pub async fn send_message(&self, output_id: &str, message: &AgentMessage) -> DoraResult<()> {
        let data = bincode::serialize(message)?;
        self.send_output(output_id, data).await
    }

    /// 注入事件到节点（供外部调度器使用）
    /// Inject event into node (for external scheduler use)
    pub async fn inject_event(&self, event: AgentEvent) -> DoraResult<()> {
        self.event_tx
            .send(event)
            .await
            .map_err(|e| DoraError::ChannelError(e.to_string()))?;
        Ok(())
    }

    /// 暂停节点
    /// Pause node
    pub async fn pause(&self) -> DoraResult<()> {
        let mut state = self.state.write().await;
        if *state == NodeState::Running {
            *state = NodeState::Paused;
            info!("DoraAgentNode {} paused", self.config.node_id);
        }
        Ok(())
    }

    /// 恢复节点
    /// Resume node
    pub async fn resume(&self) -> DoraResult<()> {
        let mut state = self.state.write().await;
        if *state == NodeState::Paused {
            *state = NodeState::Running;
            info!("DoraAgentNode {} resumed", self.config.node_id);
        }
        Ok(())
    }

    /// 停止节点
    /// Stop node
    pub async fn stop(&self) -> DoraResult<()> {
        let mut state = self.state.write().await;
        *state = NodeState::Stopping;
        self.interrupt.trigger();
        *state = NodeState::Stopped;
        info!("DoraAgentNode {} stopped", self.config.node_id);
        Ok(())
    }

    /// 创建事件循环（供智能体使用）
    /// Create event loop (for agent use)
    pub fn create_event_loop(&self) -> NodeEventLoop {
        NodeEventLoop {
            event_rx: self.event_rx.clone(),
            interrupt: self.interrupt.clone(),
            state: self.state.clone(),
        }
    }
}

/// 节点事件循环
/// Node Event Loop
pub struct NodeEventLoop {
    event_rx: Arc<RwLock<mpsc::Receiver<AgentEvent>>>,
    interrupt: AgentInterrupt,
    state: Arc<RwLock<NodeState>>,
}

impl NodeEventLoop {
    /// 获取下一个事件（阻塞）
    /// Get the next event (blocking)
    pub async fn next_event(&self) -> Option<AgentEvent> {
        // 检查中断
        // Check for interrupt
        if self.interrupt.check() {
            return Some(AgentEvent::Shutdown);
        }

        // 检查状态
        // Check state
        let state = self.state.read().await;
        if *state == NodeState::Stopped || *state == NodeState::Stopping {
            return Some(AgentEvent::Shutdown);
        }
        drop(state);

        // 接收事件
        // Receive event
        let mut event_rx = self.event_rx.write().await;
        tokio::select! {
            event = event_rx.recv() => event,
            _ = self.interrupt.notify.notified() => Some(AgentEvent::Shutdown),
        }
    }

    /// 尝试获取下一个事件（非阻塞）
    /// Try to get the next event (non-blocking)
    pub async fn try_next_event(&self) -> Option<AgentEvent> {
        if self.interrupt.check() {
            return Some(AgentEvent::Shutdown);
        }

        let mut event_rx = self.event_rx.write().await;
        event_rx.try_recv().ok()
    }

    /// 检查是否应中断
    /// Check if should interrupt
    pub fn should_interrupt(&self) -> bool {
        self.interrupt.check()
    }

    /// 获取中断句柄
    /// Get interrupt handle
    pub fn interrupt(&self) -> &AgentInterrupt {
        &self.interrupt
    }
}

/// 从 dora-rs ArrowData 提取字节数据
/// Extract byte data from dora-rs ArrowData
fn extract_bytes_from_arrow_data(data: &dora_node_api::ArrowData) -> Vec<u8> {
    // 尝试将 ArrowData 转换为 Vec<u8>
    // Attempt to convert ArrowData to Vec<u8>
    Vec::<u8>::try_from(data).unwrap_or_default()
}

/// dora-rs 原生事件到 AgentEvent 的转换
/// Conversion from dora-rs native events to AgentEvent
pub fn convert_dora_event(dora_event: &Event) -> Option<AgentEvent> {
    match dora_event {
        Event::Stop(_cause) => Some(AgentEvent::Shutdown),
        Event::Input {
            id,
            metadata: _,
            data,
        } => {
            // 从 ArrowData 获取字节数据
            // Get byte data from ArrowData
            let bytes = extract_bytes_from_arrow_data(data);

            // 尝试反序列化为 TaskRequest
            // Try to deserialize into TaskRequest
            if let Ok(task) = bincode::deserialize::<TaskRequest>(&bytes) {
                Some(AgentEvent::TaskReceived(task))
            } else if let Ok(msg) = bincode::deserialize::<AgentMessage>(&bytes) {
                // 尝试反序列化为 AgentMessage
                // Try to deserialize into AgentMessage
                match msg {
                    AgentMessage::Event(event) => Some(event),
                    AgentMessage::TaskRequest { task_id, content } => {
                        Some(AgentEvent::TaskReceived(TaskRequest {
                            task_id,
                            content,
                            priority: TaskPriority::Medium,
                            deadline: None,
                            metadata: HashMap::new(),
                        }))
                    }
                    _ => Some(AgentEvent::Custom(id.to_string(), bytes)),
                }
            } else {
                Some(AgentEvent::Custom(id.to_string(), bytes))
            }
        }
        Event::InputClosed { id } => {
            debug!("Input {} closed", id);
            None
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::{DoraAgentNode, DoraNodeConfig, NodeState};

    #[tokio::test]
    async fn test_node_lifecycle() {
        let config = DoraNodeConfig {
            node_id: "test_node".to_string(),
            name: "Test Node".to_string(),
            outputs: vec!["output1".to_string()],
            ..Default::default()
        };

        let node = DoraAgentNode::new(config);
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
}
