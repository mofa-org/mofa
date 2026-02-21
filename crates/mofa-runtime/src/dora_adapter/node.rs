//! DoraNode 封装
//!
//! 封装 dora-rs 的 Node API，提供智能体生命周期管理

use crate::AgentMessage;
use crate::dora_adapter::error::{DoraError, DoraResult};
use crate::interrupt::AgentInterrupt;
use dora_node_api::Event;
use mofa_kernel::message::{AgentEvent, TaskPriority, TaskRequest};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{RwLock, mpsc};
use tracing::{debug, info, warn};

/// DoraNode 配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DoraNodeConfig {
    /// 节点唯一标识
    pub node_id: String,
    /// 节点名称
    pub name: String,
    /// 输入端口列表
    pub inputs: Vec<String>,
    /// 输出端口列表
    pub outputs: Vec<String>,
    /// 事件缓冲区大小
    pub event_buffer_size: usize,
    /// 默认超时时间
    pub default_timeout: Duration,
    /// 自定义配置
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
pub struct DoraAgentNode {
    config: DoraNodeConfig,
    state: Arc<RwLock<NodeState>>,
    interrupt: AgentInterrupt,
    /// 内部事件发送器
    event_tx: mpsc::Sender<AgentEvent>,
    /// 内部事件接收器
    event_rx: Arc<RwLock<mpsc::Receiver<AgentEvent>>>,
    /// 输出通道映射
    output_channels: Arc<RwLock<HashMap<String, mpsc::Sender<Vec<u8>>>>>,
}

impl DoraAgentNode {
    /// 创建新的 DoraAgentNode
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
    pub fn config(&self) -> &DoraNodeConfig {
        &self.config
    }

    /// 获取节点状态
    pub async fn state(&self) -> NodeState {
        self.state.read().await.clone()
    }

    /// 获取中断句柄
    pub fn interrupt(&self) -> &AgentInterrupt {
        &self.interrupt
    }

    /// 初始化节点（模拟 dora-rs 节点初始化）
    pub async fn init(&self) -> DoraResult<()> {
        let mut state = self.state.write().await;
        if *state != NodeState::Created {
            return Err(DoraError::NodeInitError(
                "Node already initialized".to_string(),
            ));
        }
        *state = NodeState::Initializing;

        // 初始化输出通道
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
    pub async fn send_message(&self, output_id: &str, message: &AgentMessage) -> DoraResult<()> {
        let data = bincode::serialize(message)?;
        self.send_output(output_id, data).await
    }

    /// 注入事件到节点（供外部调度器使用）
    pub async fn inject_event(&self, event: AgentEvent) -> DoraResult<()> {
        self.event_tx
            .send(event)
            .await
            .map_err(|e| DoraError::ChannelError(e.to_string()))?;
        Ok(())
    }

    /// 暂停节点
    pub async fn pause(&self) -> DoraResult<()> {
        let mut state = self.state.write().await;
        if *state == NodeState::Running {
            *state = NodeState::Paused;
            info!("DoraAgentNode {} paused", self.config.node_id);
        }
        Ok(())
    }

    /// 恢复节点
    pub async fn resume(&self) -> DoraResult<()> {
        let mut state = self.state.write().await;
        if *state == NodeState::Paused {
            *state = NodeState::Running;
            info!("DoraAgentNode {} resumed", self.config.node_id);
        }
        Ok(())
    }

    /// 停止节点
    pub async fn stop(&self) -> DoraResult<()> {
        let mut state = self.state.write().await;
        *state = NodeState::Stopping;
        self.interrupt.trigger();
        *state = NodeState::Stopped;
        info!("DoraAgentNode {} stopped", self.config.node_id);
        Ok(())
    }

    /// 创建事件循环（供智能体使用）
    pub fn create_event_loop(&self) -> NodeEventLoop {
        NodeEventLoop {
            event_rx: self.event_rx.clone(),
            interrupt: self.interrupt.clone(),
            state: self.state.clone(),
        }
    }
}

/// 节点事件循环
pub struct NodeEventLoop {
    event_rx: Arc<RwLock<mpsc::Receiver<AgentEvent>>>,
    interrupt: AgentInterrupt,
    state: Arc<RwLock<NodeState>>,
}

impl NodeEventLoop {
    /// 获取下一个事件（阻塞）
    pub async fn next_event(&self) -> Option<AgentEvent> {
        // 检查中断
        if self.interrupt.check() {
            return Some(AgentEvent::Shutdown);
        }

        // 检查状态
        let state = self.state.read().await;
        if *state == NodeState::Stopped || *state == NodeState::Stopping {
            return Some(AgentEvent::Shutdown);
        }
        drop(state);

        // 接收事件
        let mut event_rx = self.event_rx.write().await;
        tokio::select! {
            event = event_rx.recv() => event,
            _ = self.interrupt.notify.notified() => Some(AgentEvent::Shutdown),
        }
    }

    /// 尝试获取下一个事件（非阻塞）
    pub async fn try_next_event(&self) -> Option<AgentEvent> {
        if self.interrupt.check() {
            return Some(AgentEvent::Shutdown);
        }

        let mut event_rx = self.event_rx.write().await;
        event_rx.try_recv().ok()
    }

    /// 检查是否应中断
    pub fn should_interrupt(&self) -> bool {
        self.interrupt.check()
    }

    /// 获取中断句柄
    pub fn interrupt(&self) -> &AgentInterrupt {
        &self.interrupt
    }
}

/// 从 dora-rs ArrowData 提取字节数据
fn extract_bytes_from_arrow_data(data: &dora_node_api::ArrowData) -> Vec<u8> {
    // 尝试将 ArrowData 转换为 Vec<u8>
    Vec::<u8>::try_from(data).unwrap_or_default()
}

/// dora-rs 原生事件到 AgentEvent 的转换
pub fn convert_dora_event(dora_event: &Event) -> Option<AgentEvent> {
    match dora_event {
        Event::Stop(_cause) => Some(AgentEvent::Shutdown),
        Event::Input {
            id,
            metadata: _,
            data,
        } => {
            // 从 ArrowData 获取字节数据
            let bytes = extract_bytes_from_arrow_data(data);

            // 尝试反序列化为 TaskRequest
            if let Ok(task) = bincode::deserialize::<TaskRequest>(&bytes) {
                Some(AgentEvent::TaskReceived(task))
            } else if let Ok(msg) = bincode::deserialize::<AgentMessage>(&bytes) {
                // 尝试反序列化为 AgentMessage
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
    use super::*;

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
