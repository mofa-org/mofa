#[cfg(feature = "monitoring")]
pub use mofa_monitoring::*;

// =============================================================================
// MoFA Runtime - Agent Lifecycle and Execution Management
// =============================================================================
//
// This module provides runtime infrastructure for managing agent execution.
// It follows microkernel architecture principles by depending only on the
// kernel layer for core abstractions.
//
// Main Components:
// - AgentBuilder: Builder pattern for constructing agents
// - SimpleRuntime: Multi-agent coordination (non-dora mode)
// - AgentRuntime: Dora-rs integration (with `dora` feature)
// - run_agent: Simplified agent execution helper
//
// =============================================================================

pub mod agent;
pub mod builder;
pub mod config;
pub mod interrupt;
pub mod runner;

// Dora adapter module (only compiled when dora feature is enabled)
#[cfg(feature = "dora")]
pub mod dora_adapter;

// =============================================================================
// Re-exports from Kernel (minimal, only what runtime needs)
// =============================================================================
//
// Runtime needs these core types from kernel for its functionality:
// - MoFAAgent: Core agent trait that runtime executes
// - AgentConfig: Configuration structure
// - AgentMetadata: Agent metadata
// - AgentEvent, AgentMessage: Event and message types
// - AgentPlugin: Plugin trait for extensibility
//
// These are re-exported for user convenience when working with runtime APIs.
// =============================================================================

pub use interrupt::*;

// Core agent trait - runtime executes agents implementing this trait
pub use mofa_kernel::agent::MoFAAgent;

pub use mofa_kernel::agent::AgentMetadata;
// Core types needed for runtime operations
pub use mofa_kernel::core::AgentConfig;
pub use mofa_kernel::message::{AgentEvent, AgentMessage};

// Plugin system - runtime supports plugins
pub use mofa_kernel::plugin::AgentPlugin;

// Import from mofa-foundation
// Import from mofa-kernel

// Import from mofa-plugins
use mofa_plugins::AgentPlugin as PluginAgent;

// External dependencies
use async_trait::async_trait;
use std::collections::HashMap;
use std::time::Duration;

// Dora feature dependencies
#[cfg(feature = "dora")]
use crate::dora_adapter::{
    ChannelConfig, DataflowConfig, DoraAgentNode, DoraChannel, DoraDataflow, DoraError,
    DoraNodeConfig, DoraResult, MessageEnvelope,
};
#[cfg(feature = "dora")]
use std::sync::Arc;
#[cfg(feature = "dora")]
use tokio::sync::RwLock;
#[cfg(feature = "dora")]
use tracing::{debug, info};

// Private import for internal use
use mofa_kernel::message::StreamType;

/// 智能体构建器 - 提供流式 API
pub struct AgentBuilder {
    agent_id: String,
    name: String,
    capabilities: Vec<String>,
    dependencies: Vec<String>,
    plugins: Vec<Box<dyn PluginAgent>>,
    node_config: HashMap<String, String>,
    inputs: Vec<String>,
    outputs: Vec<String>,
    max_concurrent_tasks: usize,
    default_timeout: Duration,
}
// ------------------------------
// 简化的 SDK API
// ------------------------------

/// 运行智能体的简化接口
///
/// # 示例
/// ```rust
/// use mofa_runtime::{MoFAAgent, run_agent};
///
/// struct MyAgent { /* ... */ }
///
/// #[async_trait::async_trait]
/// impl MoFAAgent for MyAgent { /* ... */ }
///
/// #[tokio::main]
/// async fn main() -> anyhow::Result<()> {
///     let agent = MyAgent::new();
///     run_agent(agent).await
/// }
/// ```
pub async fn run_agent<A: MoFAAgent>(agent: A) -> anyhow::Result<()> {
    let builder = AgentBuilder::new(
        agent.id(),
        agent.name(),
    );

    // 使用 with_agent 需要所有权，所以我们直接使用内部 API
    let mut runtime = builder.with_agent(agent).await?;

    runtime.start().await?;

    // 等待中断
    tokio::signal::ctrl_c().await?;

    runtime.stop().await?;
    Ok(())
}

impl AgentBuilder {
    /// 创建新的 AgentBuilder
    pub fn new(agent_id: &str, name: &str) -> Self {
        Self {
            agent_id: agent_id.to_string(),
            name: name.to_string(),
            capabilities: Vec::new(),
            dependencies: Vec::new(),
            plugins: Vec::new(),
            node_config: HashMap::new(),
            inputs: vec!["task_input".to_string()],
            outputs: vec!["task_output".to_string()],
            max_concurrent_tasks: 10,
            default_timeout: Duration::from_secs(30),
        }
    }

    /// 添加能力
    pub fn with_capability(mut self, capability: &str) -> Self {
        self.capabilities.push(capability.to_string());
        self
    }

    /// 添加多个能力
    pub fn with_capabilities(mut self, capabilities: Vec<&str>) -> Self {
        for cap in capabilities {
            self.capabilities.push(cap.to_string());
        }
        self
    }

    /// 添加依赖
    pub fn with_dependency(mut self, dependency: &str) -> Self {
        self.dependencies.push(dependency.to_string());
        self
    }

    /// 添加插件
    pub fn with_plugin(mut self, plugin: Box<dyn AgentPlugin>) -> Self {
        self.plugins.push(plugin);
        self
    }

    /// 添加输入端口
    pub fn with_input(mut self, input: &str) -> Self {
        self.inputs.push(input.to_string());
        self
    }

    /// 添加输出端口
    pub fn with_output(mut self, output: &str) -> Self {
        self.outputs.push(output.to_string());
        self
    }

    /// 设置最大并发任务数
    pub fn with_max_concurrent_tasks(mut self, max: usize) -> Self {
        self.max_concurrent_tasks = max;
        self
    }

    /// 设置默认超时
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.default_timeout = timeout;
        self
    }

    /// 添加自定义配置
    pub fn with_config(mut self, key: &str, value: &str) -> Self {
        self.node_config.insert(key.to_string(), value.to_string());
        self
    }

    /// 构建智能体配置
    pub fn build_config(&self) -> AgentConfig {
        AgentConfig {
            agent_id: self.agent_id.clone(),
            name: self.name.clone(),
            node_config: self.node_config.clone(),
        }
    }

    /// 构建元数据
    pub fn build_metadata(&self) -> AgentMetadata {
        use mofa_kernel::agent::AgentCapabilities;
        use mofa_kernel::agent::AgentState;

        // 将 Vec<String> 转换为 AgentCapabilities
        let agent_capabilities = AgentCapabilities::builder()
            .tags(self.capabilities.clone())
            .build();

        AgentMetadata {
            id: self.agent_id.clone(),
            name: self.name.clone(),
            description: None,
            version: None,
            capabilities: agent_capabilities,
            state: AgentState::Created,
        }
    }

    /// 构建 DoraNodeConfig
    #[cfg(feature = "dora")]
    pub fn build_node_config(&self) -> DoraNodeConfig {
        DoraNodeConfig {
            node_id: self.agent_id.clone(),
            name: self.name.clone(),
            inputs: self.inputs.clone(),
            outputs: self.outputs.clone(),
            event_buffer_size: self.max_concurrent_tasks * 10,
            default_timeout: self.default_timeout,
            custom_config: self.node_config.clone(),
        }
    }

    /// 使用提供的 MoFAAgent 实现构建运行时
    #[cfg(feature = "dora")]
    pub async fn with_agent<A: MoFAAgent>(self, agent: A) -> DoraResult<AgentRuntime<A>> {
        let node_config = self.build_node_config();
        let metadata = self.build_metadata();
        let config = self.build_config();

        let node = DoraAgentNode::new(node_config);
        let interrupt = node.interrupt().clone();

        Ok(AgentRuntime {
            agent,
            node: Arc::new(node),
            metadata,
            config,
            interrupt,
            plugins: self.plugins,
        })
    }

    /// 构建并启动智能体（需要提供 MoFAAgent 实现）
    #[cfg(feature = "dora")]
    pub async fn build_and_start<A: MoFAAgent>(self, agent: A) -> DoraResult<AgentRuntime<A>> {
        let runtime: AgentRuntime<A> = self.with_agent(agent).await?;
        runtime.start().await?;
        Ok(runtime)
    }

    /// 使用提供的 MoFAAgent 实现构建简单运行时（非 dora 模式）
    #[cfg(not(feature = "dora"))]
    pub async fn with_agent<A: MoFAAgent>(self, agent: A) -> anyhow::Result<SimpleAgentRuntime<A>> {
        let metadata = self.build_metadata();
        let config = self.build_config();
        let interrupt = AgentInterrupt::new();

        // 创建事件通道
        let (event_tx, event_rx) = tokio::sync::mpsc::channel(100);

        Ok(SimpleAgentRuntime {
            agent,
            metadata,
            config,
            interrupt,
            plugins: self.plugins,
            inputs: self.inputs,
            outputs: self.outputs,
            max_concurrent_tasks: self.max_concurrent_tasks,
            default_timeout: self.default_timeout,
            event_tx,
            event_rx: Some(event_rx),
        })
    }

    /// 构建并启动智能体（非 dora 模式）
    #[cfg(not(feature = "dora"))]
    pub async fn build_and_start<A: MoFAAgent>(
        self,
        agent: A,
    ) -> anyhow::Result<SimpleAgentRuntime<A>> {
        let mut runtime = self.with_agent(agent).await?;
        runtime.start().await?;
        Ok(runtime)
    }
}

/// 智能体运行时
#[cfg(feature = "dora")]
pub struct AgentRuntime<A: MoFAAgent> {
    agent: A,
    node: Arc<DoraAgentNode>,
    metadata: AgentMetadata,
    config: AgentConfig,
    interrupt: AgentInterrupt,
    plugins: Vec<Box<dyn AgentPlugin>>,
}

#[cfg(feature = "dora")]
impl<A: MoFAAgent> AgentRuntime<A> {
    /// 获取智能体引用
    pub fn agent(&self) -> &A {
        &self.agent
    }

    /// 获取可变智能体引用
    pub fn agent_mut(&mut self) -> &mut A {
        &mut self.agent
    }

    /// 获取节点
    pub fn node(&self) -> &Arc<DoraAgentNode> {
        &self.node
    }

    /// 获取元数据
    pub fn metadata(&self) -> &AgentMetadata {
        &self.metadata
    }

    /// 获取配置
    pub fn config(&self) -> &AgentConfig {
        &self.config
    }

    /// 获取中断句柄
    pub fn interrupt(&self) -> &AgentInterrupt {
        &self.interrupt
    }

    /// 初始化插件
    pub async fn init_plugins(&mut self) -> DoraResult<()> {
        for plugin in &mut self.plugins {
            plugin
                .init_plugin()
                .await
                .map_err(|e| DoraError::OperatorError(e.to_string()))?;
        }
        Ok(())
    }

    /// 启动运行时
    pub async fn start(&self) -> DoraResult<()> {
        self.node.init().await?;
        info!("AgentRuntime {} started", self.metadata.id);
        Ok(())
    }

    /// 运行事件循环
    pub async fn run_event_loop(&mut self) -> DoraResult<()> {
        // 创建 AgentContext 并初始化智能体
        let context = mofa_kernel::agent::AgentContext::new(self.metadata.id.clone());
        self.agent
            .initialize(&context)
            .await
            .map_err(|e| DoraError::Internal(e.to_string()))?;

        // 初始化插件
        self.init_plugins().await?;

        let event_loop = self.node.create_event_loop();

        loop {
            // 检查中断
            if event_loop.should_interrupt() {
                debug!("Interrupt signal received for {}", self.metadata.id);
                self.interrupt.reset();
            }

            // 获取下一个事件
            match event_loop.next_event().await {
                Some(AgentEvent::Shutdown) => {
                    info!("Received shutdown event");
                    break;
                }
                Some(event) => {
                    // 处理事件前检查中断
                    if self.interrupt.check() {
                        debug!("Interrupt signal received for {}", self.metadata.id);
                        self.interrupt.reset();
                    }

                    // 将事件转换为输入并使用 execute
                    use mofa_kernel::agent::types::AgentInput;
                    use mofa_kernel::message::TaskRequest;

                    let input = match event.clone() {
                        AgentEvent::TaskReceived(task) => AgentInput::text(task.content),
                        AgentEvent::Custom(data, _) => AgentInput::text(data),
                        _ => AgentInput::text(format!("{:?}", event)),
                    };

                    self.agent
                        .execute(input, &context)
                        .await
                        .map_err(|e| DoraError::Internal(e.to_string()))?;
                }
                None => {
                    // 无事件，继续等待
                    tokio::time::sleep(Duration::from_millis(10)).await;
                }
            }
        }

        // 关闭智能体
        self.agent
            .shutdown()
            .await
            .map_err(|e| DoraError::Internal(e.to_string()))?;

        Ok(())
    }

    /// 停止运行时
    pub async fn stop(&self) -> DoraResult<()> {
        self.interrupt.trigger();
        self.node.stop().await?;
        info!("AgentRuntime {} stopped", self.metadata.id);
        Ok(())
    }

    /// 发送消息到输出
    pub async fn send_output(&self, output_id: &str, message: &AgentMessage) -> DoraResult<()> {
        self.node.send_message(output_id, message).await
    }

    /// 注入事件
    pub async fn inject_event(&self, event: AgentEvent) -> DoraResult<()> {
        self.node.inject_event(event).await
    }
}

// ============================================================================
// 非 dora 运行时实现 - SimpleAgentRuntime
// ============================================================================

/// 简单智能体运行时 - 不依赖 dora-rs 的轻量级运行时
#[cfg(not(feature = "dora"))]
pub struct SimpleAgentRuntime<A: MoFAAgent> {
    agent: A,
    metadata: AgentMetadata,
    config: AgentConfig,
    interrupt: AgentInterrupt,
    plugins: Vec<Box<dyn AgentPlugin>>,
    inputs: Vec<String>,
    outputs: Vec<String>,
    max_concurrent_tasks: usize,
    default_timeout: Duration,
    // 添加事件通道
    event_tx: tokio::sync::mpsc::Sender<AgentEvent>,
    event_rx: Option<tokio::sync::mpsc::Receiver<AgentEvent>>,
}

#[cfg(not(feature = "dora"))]
impl<A: MoFAAgent> SimpleAgentRuntime<A> {
    pub async fn inject_event(&self, event: AgentEvent) {
        // 将事件发送到事件通道
        let _ = self.event_tx.send(event).await;
    }
}

#[cfg(not(feature = "dora"))]
#[cfg(not(feature = "dora"))]
impl<A: MoFAAgent> SimpleAgentRuntime<A> {
    /// 获取智能体引用
    pub fn agent(&self) -> &A {
        &self.agent
    }

    /// 获取可变智能体引用
    pub fn agent_mut(&mut self) -> &mut A {
        &mut self.agent
    }

    /// 获取元数据
    pub fn metadata(&self) -> &AgentMetadata {
        &self.metadata
    }

    /// 获取配置
    pub fn config(&self) -> &AgentConfig {
        &self.config
    }

    /// 获取中断句柄
    pub fn interrupt(&self) -> &AgentInterrupt {
        &self.interrupt
    }

    /// 获取输入端口列表
    pub fn inputs(&self) -> &[String] {
        &self.inputs
    }

    /// 获取输出端口列表
    pub fn outputs(&self) -> &[String] {
        &self.outputs
    }

    /// 获取最大并发任务数
    pub fn max_concurrent_tasks(&self) -> usize {
        self.max_concurrent_tasks
    }

    /// 获取默认超时时间
    pub fn default_timeout(&self) -> Duration {
        self.default_timeout
    }

    /// 初始化插件
    pub async fn init_plugins(&mut self) -> anyhow::Result<()> {
        for plugin in &mut self.plugins {
            plugin.init_plugin().await?;
        }
        Ok(())
    }

    /// 启动运行时
    pub async fn start(&mut self) -> anyhow::Result<()> {
        // 创建 AgentContext
        let context = mofa_kernel::agent::AgentContext::new(self.metadata.id.clone());

        // 初始化智能体 - 使用 MoFAAgent 的 initialize 方法
        self.agent.initialize(&context).await?;
        // 初始化插件
        self.init_plugins().await?;
        tracing::info!("SimpleAgentRuntime {} started", self.metadata.id);
        Ok(())
    }

    /// 处理单个事件
    pub async fn handle_event(&mut self, event: AgentEvent) -> anyhow::Result<()> {
        // 检查中断 - 注意：MoFAAgent 没有 on_interrupt 方法
        // 中断处理需要由 Agent 内部自行处理或通过 AgentMessaging 扩展
        if self.interrupt.check() {
            // 中断信号，可以选择停止或通知 agent
            tracing::debug!("Interrupt signal received for {}", self.metadata.id);
            self.interrupt.reset();
        }

        // 将事件转换为输入并使用 execute
        use mofa_kernel::agent::types::AgentInput;
        use mofa_kernel::message::TaskRequest;

        let context = mofa_kernel::agent::AgentContext::new(self.metadata.id.clone());

        // 尝试将事件转换为输入
        let input = match event {
            AgentEvent::TaskReceived(task) => AgentInput::text(task.content),
            AgentEvent::Shutdown => {
                tracing::info!("Shutdown event received for {}", self.metadata.id);
                return Ok(());
            }
            AgentEvent::Custom(data, _) => AgentInput::text(data),
            _ => AgentInput::text(format!("{:?}", event)),
        };

        let _output = self.agent.execute(input, &context).await?;
        Ok(())
    }

    /// 运行事件循环（使用内部事件接收器）
    pub async fn run(&mut self) -> anyhow::Result<()> {
        // 获取内部事件接收器
        let event_rx = self.event_rx.take()
            .ok_or_else(|| anyhow::anyhow!("Event receiver already taken"))?;

        self.run_with_receiver(event_rx).await
    }

    /// 运行事件循环（使用事件通道）
    pub async fn run_with_receiver(
        &mut self,
        mut event_rx: tokio::sync::mpsc::Receiver<AgentEvent>,
    ) -> anyhow::Result<()> {
        use mofa_kernel::agent::types::AgentInput;

        loop {
            // 检查中断
            if self.interrupt.check() {
                tracing::debug!("Interrupt signal received for {}", self.metadata.id);
                self.interrupt.reset();
            }

            // 等待事件
            match tokio::time::timeout(Duration::from_millis(100), event_rx.recv()).await {
                Ok(Some(AgentEvent::Shutdown)) => {
                    tracing::info!("Received shutdown event");
                    break;
                }
                Ok(Some(event)) => {
                    // 使用 handle_event 方法（它会将事件转换为 execute 调用）
                    self.handle_event(event).await?;
                }
                Ok(None) => {
                    // 通道关闭
                    break;
                }
                Err(_) => {
                    // 超时，继续等待
                    continue;
                }
            }
        }

        // 关闭智能体 - 使用 shutdown 而不是 destroy
        self.agent.shutdown().await?;
        Ok(())
    }

    /// 停止运行时
    pub async fn stop(&mut self) -> anyhow::Result<()> {
        self.interrupt.trigger();
        self.agent.shutdown().await?;
        tracing::info!("SimpleAgentRuntime {} stopped", self.metadata.id);
        Ok(())
    }

    /// 触发中断
    pub fn trigger_interrupt(&self) {
        self.interrupt.trigger();
    }
}

// ============================================================================
// 简单多智能体运行时 - SimpleRuntime
// ============================================================================

/// 简单运行时 - 管理多个智能体的协同运行（非 dora 版本）
#[cfg(not(feature = "dora"))]
pub struct SimpleRuntime {
    agents: std::sync::Arc<tokio::sync::RwLock<HashMap<String, SimpleAgentInfo>>>,
    agent_roles: std::sync::Arc<tokio::sync::RwLock<HashMap<String, String>>>,
    message_bus: std::sync::Arc<SimpleMessageBus>,
}

/// 智能体信息
#[cfg(not(feature = "dora"))]
pub struct SimpleAgentInfo {
    pub metadata: AgentMetadata,
    pub config: AgentConfig,
    pub event_tx: tokio::sync::mpsc::Sender<AgentEvent>,
}

/// 流状态信息
#[cfg(not(feature = "dora"))]
#[derive(Debug,Clone)]
pub struct StreamInfo {
    pub stream_id: String,
    pub stream_type: StreamType,
    pub metadata: HashMap<String, String>,
    pub subscribers: Vec<String>,
    pub sequence: u64,
    pub is_paused: bool,
}

/// 简单消息总线
#[cfg(not(feature = "dora"))]
pub struct SimpleMessageBus {
    subscribers: tokio::sync::RwLock<HashMap<String, Vec<tokio::sync::mpsc::Sender<AgentEvent>>>>,
    topic_subscribers: tokio::sync::RwLock<HashMap<String, Vec<String>>>,
    // 流支持
    streams: tokio::sync::RwLock<HashMap<String, StreamInfo>>,
}

#[cfg(not(feature = "dora"))]
impl SimpleMessageBus {
    /// 创建新的消息总线
    pub fn new() -> Self {
        Self {
            subscribers: tokio::sync::RwLock::new(HashMap::new()),
            topic_subscribers: tokio::sync::RwLock::new(HashMap::new()),
            streams: tokio::sync::RwLock::new(HashMap::new()),
        }
    }

    /// 注册智能体
    pub async fn register(&self, agent_id: &str, tx: tokio::sync::mpsc::Sender<AgentEvent>) {
        let mut subs = self.subscribers.write().await;
        subs.entry(agent_id.to_string())
            .or_insert_with(Vec::new)
            .push(tx);
    }

    /// 订阅主题
    pub async fn subscribe(&self, agent_id: &str, topic: &str) {
        let mut topics = self.topic_subscribers.write().await;
        topics
            .entry(topic.to_string())
            .or_insert_with(Vec::new)
            .push(agent_id.to_string());
    }

    /// 发送点对点消息
    pub async fn send_to(&self, target_id: &str, event: AgentEvent) -> anyhow::Result<()> {
        let subs = self.subscribers.read().await;
        if let Some(senders) = subs.get(target_id) {
            for tx in senders {
                let _ = tx.send(event.clone()).await;
            }
        }
        Ok(())
    }

    /// 广播消息给所有智能体
    pub async fn broadcast(&self, event: AgentEvent) -> anyhow::Result<()> {
        let subs = self.subscribers.read().await;
        for senders in subs.values() {
            for tx in senders {
                let _ = tx.send(event.clone()).await;
            }
        }
        Ok(())
    }

    /// 发布到主题
    pub async fn publish(&self, topic: &str, event: AgentEvent) -> anyhow::Result<()> {
        let topics = self.topic_subscribers.read().await;
        if let Some(agent_ids) = topics.get(topic) {
            let subs = self.subscribers.read().await;
            for agent_id in agent_ids {
                if let Some(senders) = subs.get(agent_id) {
                    for tx in senders {
                        let _ = tx.send(event.clone()).await;
                    }
                }
            }
        }
        Ok(())
    }

    // ---------------------------------
    // 流支持方法
    // ---------------------------------

    /// 创建流
    pub async fn create_stream(
        &self,
        stream_id: &str,
        stream_type: StreamType,
        metadata: HashMap<String, String>,
    ) -> anyhow::Result<()> {
        let mut streams = self.streams.write().await;
        if streams.contains_key(stream_id) {
            return Err(anyhow::anyhow!("Stream {} already exists", stream_id));
        }

        // 创建流信息
        let stream_info = StreamInfo {
            stream_id: stream_id.to_string(),
            stream_type: stream_type.clone(),
            metadata: metadata.clone(),
            subscribers: Vec::new(),
            sequence: 0,
            is_paused: false,
        };

        streams.insert(stream_id.to_string(), stream_info.clone());

        // 广播流创建事件
        self.broadcast(AgentEvent::StreamCreated {
            stream_id: stream_id.to_string(),
            stream_type,
            metadata,
        })
        .await
    }

    /// 关闭流
    pub async fn close_stream(&self, stream_id: &str, reason: &str) -> anyhow::Result<()> {
        let mut streams = self.streams.write().await;
        if let Some(stream_info) = streams.remove(stream_id) {
            // 广播流关闭事件
            let event = AgentEvent::StreamClosed {
                stream_id: stream_id.to_string(),
                reason: reason.to_string(),
            };

            // 通知所有订阅者
            let subs = self.subscribers.read().await;
            for agent_id in &stream_info.subscribers {
                if let Some(senders) = subs.get(agent_id) {
                    for tx in senders {
                        let _ = tx.send(event.clone()).await;
                    }
                }
            }
        }
        Ok(())
    }

    /// 订阅流
    pub async fn subscribe_stream(&self, agent_id: &str, stream_id: &str) -> anyhow::Result<()> {
        let mut streams = self.streams.write().await;
        if let Some(stream_info) = streams.get_mut(stream_id) {
            // 检查是否已订阅
            if !stream_info.subscribers.contains(&agent_id.to_string()) {
                stream_info.subscribers.push(agent_id.to_string());

                // 广播订阅事件
                self.broadcast(AgentEvent::StreamSubscription {
                    stream_id: stream_id.to_string(),
                    subscriber_id: agent_id.to_string(),
                })
                .await?;
            }
        }
        Ok(())
    }

    /// 取消订阅流
    pub async fn unsubscribe_stream(&self, agent_id: &str, stream_id: &str) -> anyhow::Result<()> {
        let mut streams = self.streams.write().await;
        if let Some(stream_info) = streams.get_mut(stream_id) {
            // 移除订阅者
            if let Some(pos) = stream_info.subscribers.iter().position(|id| id == agent_id) {
                stream_info.subscribers.remove(pos);

                // 广播取消订阅事件
                self.broadcast(AgentEvent::StreamUnsubscription {
                    stream_id: stream_id.to_string(),
                    subscriber_id: agent_id.to_string(),
                })
                .await?;
            }
        }
        Ok(())
    }

    /// 发送流消息
    pub async fn send_stream_message(
        &self,
        stream_id: &str,
        message: Vec<u8>,
    ) -> anyhow::Result<()> {
        let mut streams = self.streams.write().await;
        if let Some(stream_info) = streams.get_mut(stream_id) {
            // 如果流被暂停，直接返回
            if stream_info.is_paused {
                return Ok(());
            }

            // 生成序列号
            let sequence = stream_info.sequence;
            stream_info.sequence += 1;

            // 构造流消息事件
            let event = AgentEvent::StreamMessage {
                stream_id: stream_id.to_string(),
                message,
                sequence,
            };

            // 发送给所有订阅者
            let subs = self.subscribers.read().await;
            for agent_id in &stream_info.subscribers {
                if let Some(senders) = subs.get(agent_id) {
                    for tx in senders {
                        let _ = tx.send(event.clone()).await;
                    }
                }
            }
        }
        Ok(())
    }

    /// 暂停流
    pub async fn pause_stream(&self, stream_id: &str) -> anyhow::Result<()> {
        let mut streams = self.streams.write().await;
        if let Some(stream_info) = streams.get_mut(stream_id) {
            stream_info.is_paused = true;
        }
        Ok(())
    }

    /// 恢复流
    pub async fn resume_stream(&self, stream_id: &str) -> anyhow::Result<()> {
        let mut streams = self.streams.write().await;
        if let Some(stream_info) = streams.get_mut(stream_id) {
            stream_info.is_paused = false;
        }
        Ok(())
    }

    /// 获取流信息
    pub async fn get_stream_info(&self, stream_id: &str) -> anyhow::Result<Option<StreamInfo>> {
        let streams = self.streams.read().await;
        Ok(streams.get(stream_id).cloned())
    }
}

#[cfg(not(feature = "dora"))]
impl Default for SimpleMessageBus {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(not(feature = "dora"))]
impl SimpleRuntime {
    /// 创建新的简单运行时
    pub fn new() -> Self {
        Self {
            agents: std::sync::Arc::new(tokio::sync::RwLock::new(HashMap::new())),
            agent_roles: std::sync::Arc::new(tokio::sync::RwLock::new(HashMap::new())),
            message_bus: std::sync::Arc::new(SimpleMessageBus::new()),
        }
    }

    /// 注册智能体
    pub async fn register_agent(
        &self,
        metadata: AgentMetadata,
        config: AgentConfig,
        role: &str,
    ) -> anyhow::Result<tokio::sync::mpsc::Receiver<AgentEvent>> {
        let agent_id = metadata.id.clone();
        let (tx, rx) = tokio::sync::mpsc::channel(100);

        // 注册到消息总线
        self.message_bus.register(&agent_id, tx.clone()).await;

        // 添加智能体信息
        let mut agents = self.agents.write().await;
        agents.insert(
            agent_id.clone(),
            SimpleAgentInfo {
                metadata,
                config,
                event_tx: tx,
            },
        );

        // 记录角色
        let mut roles = self.agent_roles.write().await;
        roles.insert(agent_id.clone(), role.to_string());

        tracing::info!("Agent {} registered with role {}", agent_id, role);
        Ok(rx)
    }

    /// 获取消息总线
    pub fn message_bus(&self) -> &std::sync::Arc<SimpleMessageBus> {
        &self.message_bus
    }

    /// 获取指定角色的智能体列表
    pub async fn get_agents_by_role(&self, role: &str) -> Vec<String> {
        let roles = self.agent_roles.read().await;
        roles
            .iter()
            .filter(|(_, r)| *r == role)
            .map(|(id, _)| id.clone())
            .collect()
    }

    /// 发送消息给指定智能体
    pub async fn send_to_agent(&self, target_id: &str, event: AgentEvent) -> anyhow::Result<()> {
        self.message_bus.send_to(target_id, event).await
    }

    /// 广播消息给所有智能体
    pub async fn broadcast(&self, event: AgentEvent) -> anyhow::Result<()> {
        self.message_bus.broadcast(event).await
    }

    /// 发布到主题
    pub async fn publish_to_topic(&self, topic: &str, event: AgentEvent) -> anyhow::Result<()> {
        self.message_bus.publish(topic, event).await
    }

    /// 订阅主题
    pub async fn subscribe_topic(&self, agent_id: &str, topic: &str) -> anyhow::Result<()> {
        self.message_bus.subscribe(agent_id, topic).await;
        Ok(())
    }

    // ---------------------------------
    // 流支持方法
    // ---------------------------------

    /// 创建流
    pub async fn create_stream(
        &self,
        stream_id: &str,
        stream_type: StreamType,
        metadata: std::collections::HashMap<String, String>,
    ) -> anyhow::Result<()> {
        self.message_bus
            .create_stream(stream_id, stream_type, metadata)
            .await
    }

    /// 关闭流
    pub async fn close_stream(&self, stream_id: &str, reason: &str) -> anyhow::Result<()> {
        self.message_bus.close_stream(stream_id, reason).await
    }

    /// 订阅流
    pub async fn subscribe_stream(&self, agent_id: &str, stream_id: &str) -> anyhow::Result<()> {
        self.message_bus.subscribe_stream(agent_id, stream_id).await
    }

    /// 取消订阅流
    pub async fn unsubscribe_stream(&self, agent_id: &str, stream_id: &str) -> anyhow::Result<()> {
        self.message_bus.unsubscribe_stream(agent_id, stream_id).await
    }

    /// 发送流消息
    pub async fn send_stream_message(&self, stream_id: &str, message: Vec<u8>) -> anyhow::Result<()> {
        self.message_bus.send_stream_message(stream_id, message).await
    }

    /// 暂停流
    pub async fn pause_stream(&self, stream_id: &str) -> anyhow::Result<()> {
        self.message_bus.pause_stream(stream_id).await
    }

    /// 恢复流
    pub async fn resume_stream(&self, stream_id: &str) -> anyhow::Result<()> {
        self.message_bus.resume_stream(stream_id).await
    }

    /// 获取流信息
    pub async fn get_stream_info(&self, stream_id: &str) -> anyhow::Result<Option<StreamInfo>> {
        self.message_bus.get_stream_info(stream_id).await
    }

    /// 停止所有智能体
    pub async fn stop_all(&self) -> anyhow::Result<()> {
        self.message_bus.broadcast(AgentEvent::Shutdown).await?;
        tracing::info!("SimpleRuntime stopped");
        Ok(())
    }
}

#[cfg(not(feature = "dora"))]
impl Default for SimpleRuntime {
    fn default() -> Self {
        Self::new()
    }
}

/// 智能体节点存储类型
#[cfg(feature = "dora")]
type AgentNodeMap = HashMap<String, Arc<DoraAgentNode>>;

/// MoFA 运行时 - 管理多个智能体的协同运行
#[cfg(feature = "dora")]
pub struct MoFARuntime {
    dataflow: Option<DoraDataflow>,
    channel: Arc<DoraChannel>,
    agents: Arc<RwLock<AgentNodeMap>>,
    agent_roles: Arc<RwLock<HashMap<String, String>>>,
}

#[cfg(feature = "dora")]
impl MoFARuntime {
    /// 创建新的运行时
    pub async fn new() -> Self {
        let channel_config = ChannelConfig::default();
        Self {
            dataflow: None,
            channel: Arc::new(DoraChannel::new(channel_config)),
            agents: Arc::new(RwLock::new(HashMap::new())),
            agent_roles: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// 使用 Dataflow 配置创建运行时
    pub async fn with_dataflow(dataflow_config: DataflowConfig) -> Self {
        let dataflow = DoraDataflow::new(dataflow_config);
        let channel_config = ChannelConfig::default();
        Self {
            dataflow: Some(dataflow),
            channel: Arc::new(DoraChannel::new(channel_config)),
            agents: Arc::new(RwLock::new(HashMap::new())),
            agent_roles: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// 注册智能体节点
    pub async fn register_agent(&self, node: DoraAgentNode, role: &str) -> DoraResult<()> {
        let agent_id = node.config().node_id.clone();

        // 注册到通道
        self.channel.register_agent(&agent_id).await?;

        // 添加到 dataflow（如果存在）
        if let Some(ref dataflow) = self.dataflow {
            dataflow.add_node(node).await?;
        } else {
            let mut agents: tokio::sync::RwLockWriteGuard<'_, AgentNodeMap> =
                self.agents.write().await;
            agents.insert(agent_id.clone(), Arc::new(node));
        }

        // 记录角色
        let mut roles = self.agent_roles.write().await;
        roles.insert(agent_id.clone(), role.to_string());

        info!("Agent {} registered with role {}", agent_id, role);
        Ok(())
    }

    /// 连接两个智能体
    pub async fn connect_agents(
        &self,
        source_id: &str,
        source_output: &str,
        target_id: &str,
        target_input: &str,
    ) -> DoraResult<()> {
        if let Some(ref dataflow) = self.dataflow {
            dataflow
                .connect(source_id, source_output, target_id, target_input)
                .await?;
        }
        Ok(())
    }

    /// 获取通道
    pub fn channel(&self) -> &Arc<DoraChannel> {
        &self.channel
    }

    /// 获取指定角色的智能体列表
    pub async fn get_agents_by_role(&self, role: &str) -> Vec<String> {
        let roles = self.agent_roles.read().await;
        roles
            .iter()
            .filter(|(_, r)| *r == role)
            .map(|(id, _)| id.clone())
            .collect()
    }

    /// 发送消息给指定智能体
    pub async fn send_to_agent(
        &self,
        sender_id: &str,
        receiver_id: &str,
        message: &AgentMessage,
    ) -> DoraResult<()> {
        let envelope = MessageEnvelope::from_agent_message(sender_id, message)?.to(receiver_id);
        self.channel.send_p2p(envelope).await
    }

    /// 广播消息给所有智能体
    pub async fn broadcast(&self, sender_id: &str, message: &AgentMessage) -> DoraResult<()> {
        let envelope = MessageEnvelope::from_agent_message(sender_id, message)?;
        self.channel.broadcast(envelope).await
    }

    /// 发布到主题
    pub async fn publish_to_topic(
        &self,
        sender_id: &str,
        topic: &str,
        message: &AgentMessage,
    ) -> DoraResult<()> {
        let envelope = MessageEnvelope::from_agent_message(sender_id, message)?.with_topic(topic);
        self.channel.publish(envelope).await
    }

    /// 订阅主题
    pub async fn subscribe_topic(&self, agent_id: &str, topic: &str) -> DoraResult<()> {
        self.channel.subscribe(agent_id, topic).await
    }

    /// 构建并启动运行时
    pub async fn build_and_start(&self) -> DoraResult<()> {
        if let Some(ref dataflow) = self.dataflow {
            dataflow.build().await?;
            dataflow.start().await?;
        } else {
            // 初始化所有独立注册的智能体
            let agents: tokio::sync::RwLockReadGuard<'_, AgentNodeMap> = self.agents.read().await;
            for (id, node) in agents.iter() {
                node.init().await?;
                debug!("Agent {} initialized", id);
            }
        }
        info!("MoFARuntime started");
        Ok(())
    }

    /// 停止运行时
    pub async fn stop(&self) -> DoraResult<()> {
        if let Some(ref dataflow) = self.dataflow {
            dataflow.stop().await?;
        } else {
            let agents: tokio::sync::RwLockReadGuard<'_, AgentNodeMap> = self.agents.read().await;
            for node in agents.values() {
                node.stop().await?;
            }
        }
        info!("MoFARuntime stopped");
        Ok(())
    }

    /// 暂停运行时
    pub async fn pause(&self) -> DoraResult<()> {
        if let Some(ref dataflow) = self.dataflow {
            dataflow.pause().await?;
        }
        Ok(())
    }

    /// 恢复运行时
    pub async fn resume(&self) -> DoraResult<()> {
        if let Some(ref dataflow) = self.dataflow {
            dataflow.resume().await?;
        }
        Ok(())
    }
}
