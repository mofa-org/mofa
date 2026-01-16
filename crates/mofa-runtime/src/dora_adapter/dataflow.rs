//! DoraDataflow 封装
//!
//! 封装 dora-rs 的 Dataflow 概念，提供多智能体协同数据流管理

use crate::dora_adapter::error::{DoraError, DoraResult};
use crate::dora_adapter::node::{DoraAgentNode, DoraNodeConfig};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};
use tracing::{debug, error, info};

/// Dataflow 配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataflowConfig {
    /// Dataflow 唯一标识
    pub dataflow_id: String,
    /// Dataflow 名称
    pub name: String,
    /// 默认通道缓冲区大小
    pub default_buffer_size: usize,
    /// 是否启用持久化
    pub enable_persistence: bool,
    /// 自定义配置
    pub custom_config: HashMap<String, String>,
}

impl Default for DataflowConfig {
    fn default() -> Self {
        Self {
            dataflow_id: uuid::Uuid::now_v7().to_string(),
            name: "default_dataflow".to_string(),
            default_buffer_size: 1024,
            enable_persistence: false,
            custom_config: HashMap::new(),
        }
    }
}

/// Dataflow 状态
#[derive(Debug, Clone, PartialEq)]
pub enum DataflowState {
    Created,
    Building,
    Ready,
    Running,
    Paused,
    Stopping,
    Stopped,
    Error(String),
}

/// 节点连接定义
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeConnection {
    /// 源节点 ID
    pub source_node: String,
    /// 源输出端口
    pub source_output: String,
    /// 目标节点 ID
    pub target_node: String,
    /// 目标输入端口
    pub target_input: String,
}

/// 封装 dora-rs Dataflow 的多智能体数据流
pub struct DoraDataflow {
    config: DataflowConfig,
    state: Arc<RwLock<DataflowState>>,
    /// 节点注册表
    nodes: Arc<RwLock<HashMap<String, Arc<DoraAgentNode>>>>,
    /// 连接关系
    connections: Arc<RwLock<Vec<NodeConnection>>>,
    /// 内部消息路由通道
    router_tx: mpsc::Sender<RouterMessage>,
    router_rx: Arc<RwLock<mpsc::Receiver<RouterMessage>>>,
}

/// 路由消息
#[derive(Debug, Clone)]
struct RouterMessage {
    source_node: String,
    source_output: String,
    data: Vec<u8>,
}

impl DoraDataflow {
    /// 创建新的 Dataflow
    pub fn new(config: DataflowConfig) -> Self {
        let (router_tx, router_rx) = mpsc::channel(config.default_buffer_size);
        Self {
            config,
            state: Arc::new(RwLock::new(DataflowState::Created)),
            nodes: Arc::new(RwLock::new(HashMap::new())),
            connections: Arc::new(RwLock::new(Vec::new())),
            router_tx,
            router_rx: Arc::new(RwLock::new(router_rx)),
        }
    }

    /// 获取配置
    pub fn config(&self) -> &DataflowConfig {
        &self.config
    }

    /// 获取状态
    pub async fn state(&self) -> DataflowState {
        self.state.read().await.clone()
    }

    /// 添加节点
    pub async fn add_node(&self, node: DoraAgentNode) -> DoraResult<()> {
        let state = self.state.read().await;
        if *state != DataflowState::Created && *state != DataflowState::Building {
            return Err(DoraError::DataflowError(
                "Cannot add node after dataflow is ready".to_string(),
            ));
        }
        drop(state);

        let node_id = node.config().node_id.clone();
        let mut nodes = self.nodes.write().await;
        if nodes.contains_key(&node_id) {
            return Err(DoraError::DataflowError(format!(
                "Node {} already exists",
                node_id
            )));
        }

        nodes.insert(node_id.clone(), Arc::new(node));
        info!(
            "Added node {} to dataflow {}",
            node_id, self.config.dataflow_id
        );
        Ok(())
    }

    /// 连接两个节点
    pub async fn connect(
        &self,
        source_node: &str,
        source_output: &str,
        target_node: &str,
        target_input: &str,
    ) -> DoraResult<()> {
        let state = self.state.read().await;
        if *state != DataflowState::Created && *state != DataflowState::Building {
            return Err(DoraError::DataflowError(
                "Cannot add connection after dataflow is ready".to_string(),
            ));
        }
        drop(state);

        // 验证节点存在
        let nodes = self.nodes.read().await;
        if !nodes.contains_key(source_node) {
            return Err(DoraError::DataflowError(format!(
                "Source node {} not found",
                source_node
            )));
        }
        if !nodes.contains_key(target_node) {
            return Err(DoraError::DataflowError(format!(
                "Target node {} not found",
                target_node
            )));
        }
        drop(nodes);

        let connection = NodeConnection {
            source_node: source_node.to_string(),
            source_output: source_output.to_string(),
            target_node: target_node.to_string(),
            target_input: target_input.to_string(),
        };

        let mut connections = self.connections.write().await;
        connections.push(connection);
        info!(
            "Connected {}:{} -> {}:{}",
            source_node, source_output, target_node, target_input
        );
        Ok(())
    }

    /// 构建 Dataflow（验证并准备运行）
    pub async fn build(&self) -> DoraResult<()> {
        let mut state = self.state.write().await;
        *state = DataflowState::Building;

        // 验证连接
        let nodes = self.nodes.read().await;
        let connections = self.connections.read().await;

        for conn in connections.iter() {
            if !nodes.contains_key(&conn.source_node) {
                return Err(DoraError::DataflowError(format!(
                    "Source node {} not found in connection",
                    conn.source_node
                )));
            }
            if !nodes.contains_key(&conn.target_node) {
                return Err(DoraError::DataflowError(format!(
                    "Target node {} not found in connection",
                    conn.target_node
                )));
            }
        }

        *state = DataflowState::Ready;
        info!("Dataflow {} built successfully", self.config.dataflow_id);
        Ok(())
    }

    /// 启动 Dataflow
    pub async fn start(&self) -> DoraResult<()> {
        let mut state = self.state.write().await;
        if *state != DataflowState::Ready {
            return Err(DoraError::DataflowError(
                "Dataflow not ready, call build() first".to_string(),
            ));
        }

        // 初始化所有节点
        let nodes = self.nodes.read().await;
        for (node_id, node) in nodes.iter() {
            node.init().await?;
            debug!("Node {} initialized", node_id);
        }

        *state = DataflowState::Running;
        info!("Dataflow {} started", self.config.dataflow_id);

        // 启动消息路由
        self.start_router().await;
        Ok(())
    }

    /// 启动消息路由器
    async fn start_router(&self) {
        let connections = self.connections.clone();
        let nodes = self.nodes.clone();
        let router_rx = self.router_rx.clone();

        tokio::spawn(async move {
            let mut rx = router_rx.write().await;
            while let Some(msg) = rx.recv().await {
                let conns = connections.read().await;
                let node_map = nodes.read().await;

                // 找到所有匹配的连接
                for conn in conns.iter() {
                    if conn.source_node == msg.source_node
                        && conn.source_output == msg.source_output
                    {
                        if let Some(target_node) = node_map.get(&conn.target_node) {
                            // 将数据转换为事件并注入目标节点
                            if let Ok(event) = bincode::deserialize(&msg.data) {
                                if let Err(e) = target_node.inject_event(event).await {
                                    error!("Failed to inject event to {}: {}", conn.target_node, e);
                                }
                            }
                        }
                    }
                }
            }
        });
    }

    /// 获取节点
    pub async fn get_node(&self, node_id: &str) -> Option<Arc<DoraAgentNode>> {
        let nodes = self.nodes.read().await;
        nodes.get(node_id).cloned()
    }

    /// 获取所有节点 ID
    pub async fn node_ids(&self) -> Vec<String> {
        let nodes = self.nodes.read().await;
        nodes.keys().cloned().collect()
    }

    /// 暂停 Dataflow
    pub async fn pause(&self) -> DoraResult<()> {
        let mut state = self.state.write().await;
        if *state != DataflowState::Running {
            return Err(DoraError::DataflowError("Dataflow not running".to_string()));
        }

        let nodes = self.nodes.read().await;
        for node in nodes.values() {
            node.pause().await?;
        }

        *state = DataflowState::Paused;
        info!("Dataflow {} paused", self.config.dataflow_id);
        Ok(())
    }

    /// 恢复 Dataflow
    pub async fn resume(&self) -> DoraResult<()> {
        let mut state = self.state.write().await;
        if *state != DataflowState::Paused {
            return Err(DoraError::DataflowError("Dataflow not paused".to_string()));
        }

        let nodes = self.nodes.read().await;
        for node in nodes.values() {
            node.resume().await?;
        }

        *state = DataflowState::Running;
        info!("Dataflow {} resumed", self.config.dataflow_id);
        Ok(())
    }

    /// 停止 Dataflow
    pub async fn stop(&self) -> DoraResult<()> {
        let mut state = self.state.write().await;
        *state = DataflowState::Stopping;

        let nodes = self.nodes.read().await;
        for node in nodes.values() {
            node.stop().await?;
        }

        *state = DataflowState::Stopped;
        info!("Dataflow {} stopped", self.config.dataflow_id);
        Ok(())
    }
}

/// Dataflow 构建器 - 提供流式 API
pub struct DataflowBuilder {
    config: DataflowConfig,
    nodes: Vec<DoraAgentNode>,
    connections: Vec<NodeConnection>,
}

impl DataflowBuilder {
    pub fn new(name: &str) -> Self {
        Self {
            config: DataflowConfig {
                name: name.to_string(),
                ..Default::default()
            },
            nodes: Vec::new(),
            connections: Vec::new(),
        }
    }

    /// 设置 Dataflow ID
    pub fn with_id(mut self, id: &str) -> Self {
        self.config.dataflow_id = id.to_string();
        self
    }

    /// 设置缓冲区大小
    pub fn with_buffer_size(mut self, size: usize) -> Self {
        self.config.default_buffer_size = size;
        self
    }

    /// 添加节点
    pub fn add_node(mut self, node: DoraAgentNode) -> Self {
        self.nodes.push(node);
        self
    }

    /// 添加节点配置
    pub fn add_node_config(mut self, config: DoraNodeConfig) -> Self {
        self.nodes.push(DoraAgentNode::new(config));
        self
    }

    /// 连接节点
    pub fn connect(
        mut self,
        source_node: &str,
        source_output: &str,
        target_node: &str,
        target_input: &str,
    ) -> Self {
        self.connections.push(NodeConnection {
            source_node: source_node.to_string(),
            source_output: source_output.to_string(),
            target_node: target_node.to_string(),
            target_input: target_input.to_string(),
        });
        self
    }

    /// 构建 Dataflow
    pub async fn build(self) -> DoraResult<DoraDataflow> {
        let dataflow = DoraDataflow::new(self.config);

        // 添加所有节点
        for node in self.nodes {
            dataflow.add_node(node).await?;
        }

        // 添加所有连接
        for conn in self.connections {
            dataflow
                .connect(
                    &conn.source_node,
                    &conn.source_output,
                    &conn.target_node,
                    &conn.target_input,
                )
                .await?;
        }

        // 验证并准备
        dataflow.build().await?;
        Ok(dataflow)
    }

    /// 构建并启动 Dataflow
    pub async fn build_and_start(self) -> DoraResult<DoraDataflow> {
        let dataflow = self.build().await?;
        dataflow.start().await?;
        Ok(dataflow)
    }
}

#[cfg(test)]
mod tests {
    #[tokio::test]
    async fn test_dataflow_builder() {
        let node1_config = DoraNodeConfig {
            node_id: "node1".to_string(),
            outputs: vec!["out".to_string()],
            ..Default::default()
        };
        let node2_config = DoraNodeConfig {
            node_id: "node2".to_string(),
            inputs: vec!["in".to_string()],
            ..Default::default()
        };

        let dataflow = DataflowBuilder::new("test_dataflow")
            .add_node_config(node1_config)
            .add_node_config(node2_config)
            .connect("node1", "out", "node2", "in")
            .build()
            .await
            .unwrap();

        assert_eq!(dataflow.state().await, DataflowState::Ready);
        assert_eq!(dataflow.node_ids().await.len(), 2);
    }

    #[tokio::test]
    async fn test_dataflow_lifecycle() {
        let node_config = DoraNodeConfig {
            node_id: "test_node".to_string(),
            ..Default::default()
        };

        let dataflow = DataflowBuilder::new("lifecycle_test")
            .add_node_config(node_config)
            .build_and_start()
            .await
            .unwrap();

        assert_eq!(dataflow.state().await, DataflowState::Running);

        dataflow.pause().await.unwrap();
        assert_eq!(dataflow.state().await, DataflowState::Paused);

        dataflow.resume().await.unwrap();
        assert_eq!(dataflow.state().await, DataflowState::Running);

        dataflow.stop().await.unwrap();
        assert_eq!(dataflow.state().await, DataflowState::Stopped);
    }
}
