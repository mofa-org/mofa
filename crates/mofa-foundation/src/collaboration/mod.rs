//! LLM 驱动的协作协议实现
//! LLM-driven collaboration protocol implementation
//!
//! 本模块提供 MoFA 框架的标准协作协议实现，**所有协议都可以选择性地使用 LLM**。
//! This module provides standard collaboration protocol implementations for MoFA, **all of which can optionally use LLMs**.
//!
//! # 核心理念
//! # Core Philosophy
//!
//! 在 Agent 框架中，协作协议应该能够：
//! In the Agent framework, collaboration protocols should be able to:
//! 1. 使用 LLM 来理解和处理自然语言消息
//! 1. Use LLMs to understand and process natural language messages
//! 2. 通过 LLM 进行智能决策
//! 2. Make intelligent decisions through LLMs
//! 3. 记录 LLM 的推理过程
//! 3. Record the reasoning process of the LLM

// 导出类型定义
// Export type definitions
use mofa_kernel::agent::types::error::{GlobalError, GlobalResult};
pub mod types;

// 重新导出核心类型
// Re-export core types
pub use types::*;

use crate::llm::LLMClient;
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// 获取协作模式的描述
/// Get the description of the collaboration mode
fn get_mode_description(mode: &CollaborationMode) -> &'static str {
    match mode {
        CollaborationMode::RequestResponse => "一对一的请求-响应，适合确定性任务",
        // One-to-one request-response, suitable for deterministic tasks
        CollaborationMode::PublishSubscribe => "一对多的发布订阅，适合发散性任务",
        // One-to-many publish-subscribe, suitable for divergent tasks
        CollaborationMode::Consensus => "多 Agent 共识决策，适合需要达成一致的场景",
        // Multi-agent consensus decision-making, suitable for scenarios requiring agreement
        CollaborationMode::Debate => "多轮辩论优化，适合需要迭代改进的审查任务",
        // Multi-round debate optimization, suitable for review tasks requiring iterative improvement
        CollaborationMode::Parallel => "并行处理独立任务，适合可分解的工作",
        // Parallel processing of independent tasks, suitable for decomposable work
        CollaborationMode::Sequential => "顺序执行有依赖的任务链",
        // Sequential execution of dependent task chains
        CollaborationMode::Custom(_) => "自定义模式",
        // Custom mode
    }
}

// ============================================================================
// LLM 辅助的协作协议基类
// LLM-assisted collaboration protocol base class
// ============================================================================

/// LLM 辅助协议基类
/// LLM-assisted protocol base class
///
/// 提供协议与 LLM 交互的基础设施
/// Provides infrastructure for protocol interaction with LLMs
pub struct LLMProtocolHelper {
    /// Agent ID
    /// Agent ID
    agent_id: String,
    /// LLM 客户端（可选）
    /// LLM client (optional)
    llm_client: Option<Arc<LLMClient>>,
    /// 是否使用 LLM 处理消息
    /// Whether to use LLM for message processing
    use_llm: bool,
}

impl LLMProtocolHelper {
    /// 创建新的 LLM 协议辅助器
    /// Create a new LLM protocol helper
    pub fn new(agent_id: impl Into<String>) -> Self {
        Self {
            agent_id: agent_id.into(),
            llm_client: None,
            use_llm: false,
        }
    }

    /// 设置 LLM 客户端
    /// Set the LLM client
    pub fn with_llm(mut self, llm_client: Arc<LLMClient>) -> Self {
        self.llm_client = Some(llm_client);
        self.use_llm = true;
        self
    }

    /// 启用/禁用 LLM
    /// Enable/disable LLM
    pub fn with_use_llm(mut self, use_llm: bool) -> Self {
        self.use_llm = use_llm;
        self
    }

    /// 获取 Agent ID
    /// Get the Agent ID
    pub fn agent_id(&self) -> &str {
        &self.agent_id
    }

    /// 使用 LLM 处理消息
    /// Process message using LLM
    pub async fn process_with_llm(
        &self,
        msg: &CollaborationMessage,
        system_prompt: &str,
    ) -> GlobalResult<CollaborationContent> {
        let llm_client = self
            .llm_client
            .as_ref()
            .ok_or_else(|| GlobalError::Other("LLM client not configured".to_string()))?;

        let user_prompt = format!(
            "你是 {}。收到一条协作消息：\n\n发送者: {}\n内容: {}\n\n请处理这条消息并返回响应。",
            // "You are {}. Received a collaboration message:\n\nSender: {}\nContent: {}\n\nPlease process this message and return a response."
            self.agent_id,
            msg.sender,
            msg.content.to_text()
        );

        let response = llm_client
            .chat()
            .system(system_prompt)
            .user(&user_prompt)
            .send()
            .await
            .map_err(|e| GlobalError::Other(e.to_string()))?;

        Ok(CollaborationContent::LLMResponse {
            reasoning: "通过 LLM 分析和处理协作消息".to_string(),
            // Analyzing and processing collaboration message via LLM
            conclusion: response.content().unwrap_or("").to_string(),
            data: serde_json::json!({
                "original_sender": msg.sender,
                "original_content": msg.content.to_text(),
            }),
        })
    }
}

// ============================================================================
// 请求-响应协议
// Request-Response Protocol
// ============================================================================

/// 请求-响应协作协议
/// Request-Response collaboration protocol
///
/// 可选择使用 LLM 来智能处理请求
/// Optionally uses LLM to intelligently handle requests
pub struct RequestResponseProtocol {
    helper: LLMProtocolHelper,
    message_queue: Arc<RwLock<Vec<CollaborationMessage>>>,
}

impl RequestResponseProtocol {
    /// 创建新的请求-响应协议（不使用 LLM）
    /// Create new Request-Response protocol (without LLM)
    pub fn new(agent_id: impl Into<String>) -> Self {
        Self {
            helper: LLMProtocolHelper::new(agent_id),
            message_queue: Arc::new(RwLock::new(Vec::new())),
        }
    }

    /// 创建支持 LLM 的请求-响应协议
    /// Create LLM-enabled Request-Response protocol
    pub fn with_llm(agent_id: impl Into<String>, llm_client: Arc<LLMClient>) -> Self {
        Self {
            helper: LLMProtocolHelper::new(agent_id).with_llm(llm_client),
            message_queue: Arc::new(RwLock::new(Vec::new())),
        }
    }
}

#[async_trait]
impl CollaborationProtocol for RequestResponseProtocol {
    fn name(&self) -> &str {
        "request_response"
    }

    fn description(&self) -> &str {
        "请求-响应协作协议：一对一通信，同步等待结果"
        // Request-Response protocol: one-to-one communication, waiting for results synchronously
    }

    fn applicable_scenarios(&self) -> Vec<String> {
        vec![
            "数据查询和处理".to_string(),
            // Data query and processing
            "确定性任务执行".to_string(),
            // Deterministic task execution
            "状态获取".to_string(),
            // Status retrieval
            "简单问答".to_string(),
            // Simple Q&A
        ]
    }

    fn mode(&self) -> CollaborationMode {
        CollaborationMode::RequestResponse
    }

    async fn send_message(&self, msg: CollaborationMessage) -> GlobalResult<()> {
        let mut queue = self.message_queue.write().await;
        queue.push(msg);
        Ok(())
    }

    async fn receive_message(&self) -> GlobalResult<Option<CollaborationMessage>> {
        let mut queue = self.message_queue.write().await;
        Ok(queue.pop())
    }

    async fn process_message(
        &self,
        msg: CollaborationMessage,
    ) -> GlobalResult<CollaborationResult> {
        let start = std::time::Instant::now();

        tracing::debug!(
            "[RequestResponse] {} processing message from {}",
            self.helper.agent_id(),
            msg.sender
        );

        // 如果配置了 LLM，使用 LLM 处理
        // If LLM is configured, use LLM for processing
        let content = if self.helper.use_llm {
            self.helper
                .process_with_llm(
                    &msg,
                    "你是一个协作 Agent，负责处理请求-响应模式的协作消息。请理解请求内容并提供准确的响应。",
                    // "You are a collaboration Agent responsible for Request-Response messages. Please understand the request and provide an accurate response."
                )
                .await?
        } else {
            CollaborationContent::Text(format!("已收到来自 {} 的请求并处理", msg.sender))
            // Received and processed request from {}
        };

        let duration = start.elapsed().as_millis() as u64;

        Ok(
            CollaborationResult::success(content, duration, CollaborationMode::RequestResponse)
                .with_participant(self.helper.agent_id().to_string())
                .with_participant(msg.sender),
        )
    }

    fn stats(&self) -> HashMap<String, serde_json::Value> {
        HashMap::from([
            (
                "agent_id".to_string(),
                serde_json::json!(self.helper.agent_id()),
            ),
            (
                "use_llm".to_string(),
                serde_json::json!(self.helper.use_llm),
            ),
        ])
    }
}

// ============================================================================
// 发布-订阅协议
// Publish-Subscribe Protocol
// ============================================================================

/// 发布-订阅协作协议
/// Publish-Subscribe collaboration protocol
pub struct PublishSubscribeProtocol {
    helper: LLMProtocolHelper,
    message_queue: Arc<RwLock<Vec<CollaborationMessage>>>,
    subscribed_topics: Arc<RwLock<std::collections::HashSet<String>>>,
}

impl PublishSubscribeProtocol {
    /// 创建新的发布-订阅协议（不使用 LLM）
    /// Create new Publish-Subscribe protocol (without LLM)
    pub fn new(agent_id: impl Into<String>) -> Self {
        Self {
            helper: LLMProtocolHelper::new(agent_id),
            message_queue: Arc::new(RwLock::new(Vec::new())),
            subscribed_topics: Arc::new(RwLock::new(std::collections::HashSet::new())),
        }
    }

    /// 创建支持 LLM 的发布-订阅协议
    /// Create LLM-enabled Publish-Subscribe protocol
    pub fn with_llm(agent_id: impl Into<String>, llm_client: Arc<LLMClient>) -> Self {
        Self {
            helper: LLMProtocolHelper::new(agent_id).with_llm(llm_client),
            message_queue: Arc::new(RwLock::new(Vec::new())),
            subscribed_topics: Arc::new(RwLock::new(std::collections::HashSet::new())),
        }
    }

    /// 订阅主题
    /// Subscribe to a topic
    pub async fn subscribe(&self, topic: String) -> GlobalResult<()> {
        let mut subscribed = self.subscribed_topics.write().await;
        subscribed.insert(topic.clone());
        tracing::debug!(
            "[PublishSubscribe] {} subscribed to topic: {}",
            self.helper.agent_id(),
            topic
        );
        Ok(())
    }
}

#[async_trait]
impl CollaborationProtocol for PublishSubscribeProtocol {
    fn name(&self) -> &str {
        "publish_subscribe"
    }

    fn description(&self) -> &str {
        "发布-订阅协作协议：一对多通信，适合发散性任务和创意生成"
        // Publish-Subscribe protocol: one-to-many, suitable for divergent tasks and creative generation
    }

    fn applicable_scenarios(&self) -> Vec<String> {
        vec![
            "创意生成和发散".to_string(),
            // Creative generation and divergence
            "通知广播".to_string(),
            // Notification broadcasting
            "事件传播".to_string(),
            // Event propagation
            "多人协作".to_string(),
            // Multi-person collaboration
        ]
    }

    fn mode(&self) -> CollaborationMode {
        CollaborationMode::PublishSubscribe
    }

    async fn send_message(&self, msg: CollaborationMessage) -> GlobalResult<()> {
        let mut queue = self.message_queue.write().await;
        queue.push(msg);
        Ok(())
    }

    async fn receive_message(&self) -> GlobalResult<Option<CollaborationMessage>> {
        let mut queue = self.message_queue.write().await;
        Ok(queue.pop())
    }

    async fn process_message(
        &self,
        msg: CollaborationMessage,
    ) -> GlobalResult<CollaborationResult> {
        let start = std::time::Instant::now();

        tracing::debug!(
            "[PublishSubscribe] {} processing message from {} on topic {:?}",
            self.helper.agent_id(),
            msg.sender,
            msg.topic
        );

        let content = if self.helper.use_llm {
            self.helper
                .process_with_llm(
                    &msg,
                    "你是一个协作 Agent，负责处理发布-订阅模式的协作消息。消息是发布给多个订阅者的，请提供适当的响应。",
                    // "You are a collaboration Agent handling Publish-Subscribe messages. Messages are broadcast to multiple subscribers, please provide a response."
                )
                .await?
        } else {
            CollaborationContent::Text(format!("已发布消息到主题 {:?}", msg.topic))
            // Message published to topic {:?}
        };

        let duration = start.elapsed().as_millis() as u64;

        Ok(
            CollaborationResult::success(content, duration, CollaborationMode::PublishSubscribe)
                .with_participant(self.helper.agent_id().to_string())
                .with_participant(msg.sender),
        )
    }

    fn stats(&self) -> HashMap<String, serde_json::Value> {
        let topics = self.subscribed_topics.blocking_read();
        HashMap::from([
            (
                "agent_id".to_string(),
                serde_json::json!(self.helper.agent_id()),
            ),
            (
                "use_llm".to_string(),
                serde_json::json!(self.helper.use_llm),
            ),
            (
                "subscribed_topics".to_string(),
                serde_json::json!(topics.len()),
            ),
        ])
    }
}

// ============================================================================
// 共识协议
// Consensus Protocol
// ============================================================================

/// 共识协作协议
/// Consensus collaboration protocol
pub struct ConsensusProtocol {
    helper: LLMProtocolHelper,
    message_queue: Arc<RwLock<Vec<CollaborationMessage>>>,
    threshold: f32,
}

impl ConsensusProtocol {
    /// 创建新的共识协议（不使用 LLM）
    /// Create new Consensus protocol (without LLM)
    pub fn new(agent_id: impl Into<String>) -> Self {
        Self {
            helper: LLMProtocolHelper::new(agent_id),
            message_queue: Arc::new(RwLock::new(Vec::new())),
            threshold: 0.7,
        }
    }

    /// 创建支持 LLM 的共识协议
    /// Create LLM-enabled Consensus protocol
    pub fn with_llm(agent_id: impl Into<String>, llm_client: Arc<LLMClient>) -> Self {
        Self {
            helper: LLMProtocolHelper::new(agent_id).with_llm(llm_client),
            message_queue: Arc::new(RwLock::new(Vec::new())),
            threshold: 0.7,
        }
    }
}

#[async_trait]
impl CollaborationProtocol for ConsensusProtocol {
    fn name(&self) -> &str {
        "consensus"
    }

    fn description(&self) -> &str {
        "共识协作协议：多 Agent 协商决策，适合需要达成一致的场景"
        // Consensus protocol: Multi-agent negotiation, suitable for scenarios requiring agreement
    }

    fn applicable_scenarios(&self) -> Vec<String> {
        vec![
            "决策制定".to_string(),
            // Decision making
            "投票评估".to_string(),
            // Voting and evaluation
            "方案选择".to_string(),
            // Proposal selection
            "质量评审".to_string(),
            // Quality review
        ]
    }

    fn mode(&self) -> CollaborationMode {
        CollaborationMode::Consensus
    }

    async fn send_message(&self, msg: CollaborationMessage) -> GlobalResult<()> {
        let mut queue = self.message_queue.write().await;
        queue.push(msg);
        Ok(())
    }

    async fn receive_message(&self) -> GlobalResult<Option<CollaborationMessage>> {
        let mut queue = self.message_queue.write().await;
        Ok(queue.pop())
    }

    async fn process_message(
        &self,
        msg: CollaborationMessage,
    ) -> GlobalResult<CollaborationResult> {
        let start = std::time::Instant::now();

        tracing::debug!(
            "[Consensus] {} processing message from {}",
            self.helper.agent_id(),
            msg.sender
        );

        let content = if self.helper.use_llm {
            self.helper
                .process_with_llm(
                    &msg,
                    "你是一个协作 Agent，负责处理共识机制的协作消息。请分析多方意见并提供综合判断。",
                    // "You are a collaboration Agent handling Consensus messages. Please analyze opinions and provide a comprehensive judgment."
                )
                .await?
        } else {
            CollaborationContent::Text(format!("已参与共识决策，阈值: {}", self.threshold))
            // Participated in consensus decision, threshold: {}
        };

        let duration = start.elapsed().as_millis() as u64;

        Ok(
            CollaborationResult::success(content, duration, CollaborationMode::Consensus)
                .with_participant(self.helper.agent_id().to_string())
                .with_participant(msg.sender),
        )
    }

    fn stats(&self) -> HashMap<String, serde_json::Value> {
        HashMap::from([
            (
                "agent_id".to_string(),
                serde_json::json!(self.helper.agent_id()),
            ),
            (
                "use_llm".to_string(),
                serde_json::json!(self.helper.use_llm),
            ),
            ("threshold".to_string(), serde_json::json!(self.threshold)),
        ])
    }
}

// ============================================================================
// 辩论协议
// Debate Protocol
// ============================================================================

/// 辩论协作协议
/// Debate collaboration protocol
pub struct DebateProtocol {
    helper: LLMProtocolHelper,
    message_queue: Arc<RwLock<Vec<CollaborationMessage>>>,
    max_rounds: usize,
}

impl DebateProtocol {
    /// 创建新的辩论协议（不使用 LLM）
    /// Create new Debate protocol (without LLM)
    pub fn new(agent_id: impl Into<String>) -> Self {
        Self {
            helper: LLMProtocolHelper::new(agent_id),
            message_queue: Arc::new(RwLock::new(Vec::new())),
            max_rounds: 3,
        }
    }

    /// 创建支持 LLM 的辩论协议
    /// Create LLM-enabled Debate protocol
    pub fn with_llm(agent_id: impl Into<String>, llm_client: Arc<LLMClient>) -> Self {
        Self {
            helper: LLMProtocolHelper::new(agent_id).with_llm(llm_client),
            message_queue: Arc::new(RwLock::new(Vec::new())),
            max_rounds: 3,
        }
    }
}

#[async_trait]
impl CollaborationProtocol for DebateProtocol {
    fn name(&self) -> &str {
        "debate"
    }

    fn description(&self) -> &str {
        "辩论协作协议：多轮讨论优化，适合需要迭代改进的审查任务"
        // Debate protocol: Multi-round discussion, suitable for review tasks requiring iterative improvement
    }

    fn applicable_scenarios(&self) -> Vec<String> {
        vec![
            "代码审查".to_string(),
            // Code review
            "方案优化".to_string(),
            // Solution optimization
            "争议解决".to_string(),
            // Dispute resolution
            "质量改进".to_string(),
            // Quality improvement
        ]
    }

    fn mode(&self) -> CollaborationMode {
        CollaborationMode::Debate
    }

    async fn send_message(&self, msg: CollaborationMessage) -> GlobalResult<()> {
        let mut queue = self.message_queue.write().await;
        queue.push(msg);
        Ok(())
    }

    async fn receive_message(&self) -> GlobalResult<Option<CollaborationMessage>> {
        let mut queue = self.message_queue.write().await;
        Ok(queue.pop())
    }

    async fn process_message(
        &self,
        msg: CollaborationMessage,
    ) -> GlobalResult<CollaborationResult> {
        let start = std::time::Instant::now();

        tracing::debug!(
            "[Debate] {} processing message from {}",
            self.helper.agent_id(),
            msg.sender
        );

        let content = if self.helper.use_llm {
            self.helper
                .process_with_llm(
                    &msg,
                    "你是一个协作 Agent，负责处理辩论模式的协作消息。请提供有建设性的观点和反驳。",
                    // "You are a collaboration Agent handling Debate messages. Please provide constructive viewpoints and counter-arguments."
                )
                .await?
        } else {
            CollaborationContent::Text(format!("已参与辩论，最大轮数: {}", self.max_rounds))
            // Participated in debate, max rounds: {}
        };

        let duration = start.elapsed().as_millis() as u64;

        Ok(
            CollaborationResult::success(content, duration, CollaborationMode::Debate)
                .with_participant(self.helper.agent_id().to_string())
                .with_participant(msg.sender),
        )
    }

    fn stats(&self) -> HashMap<String, serde_json::Value> {
        HashMap::from([
            (
                "agent_id".to_string(),
                serde_json::json!(self.helper.agent_id()),
            ),
            (
                "use_llm".to_string(),
                serde_json::json!(self.helper.use_llm),
            ),
            ("max_rounds".to_string(), serde_json::json!(self.max_rounds)),
        ])
    }
}

// ============================================================================
// 并行协议
// Parallel Protocol
// ============================================================================

/// 并行协作协议
/// Parallel collaboration protocol
pub struct ParallelProtocol {
    helper: LLMProtocolHelper,
    message_queue: Arc<RwLock<Vec<CollaborationMessage>>>,
    max_workers: usize,
}

impl ParallelProtocol {
    /// 创建新的并行协议（不使用 LLM）
    /// Create new Parallel protocol (without LLM)
    pub fn new(agent_id: impl Into<String>) -> Self {
        Self {
            helper: LLMProtocolHelper::new(agent_id),
            message_queue: Arc::new(RwLock::new(Vec::new())),
            max_workers: 4,
        }
    }

    /// 创建支持 LLM 的并行协议
    /// Create LLM-enabled Parallel protocol
    pub fn with_llm(agent_id: impl Into<String>, llm_client: Arc<LLMClient>) -> Self {
        Self {
            helper: LLMProtocolHelper::new(agent_id).with_llm(llm_client),
            message_queue: Arc::new(RwLock::new(Vec::new())),
            max_workers: 4,
        }
    }
}

#[async_trait]
impl CollaborationProtocol for ParallelProtocol {
    fn name(&self) -> &str {
        "parallel"
    }

    fn description(&self) -> &str {
        "并行协作协议：同时执行独立任务，适合可分解的工作"
        // Parallel protocol: Simultaneous execution of independent tasks, suitable for decomposable work
    }

    fn applicable_scenarios(&self) -> Vec<String> {
        vec![
            "数据分析".to_string(),
            // Data analysis
            "批量处理".to_string(),
            // Batch processing
            "分布式搜索".to_string(),
            // Distributed search
            "并行计算".to_string(),
            // Parallel computing
        ]
    }

    fn mode(&self) -> CollaborationMode {
        CollaborationMode::Parallel
    }

    async fn send_message(&self, msg: CollaborationMessage) -> GlobalResult<()> {
        let mut queue = self.message_queue.write().await;
        queue.push(msg);
        Ok(())
    }

    async fn receive_message(&self) -> GlobalResult<Option<CollaborationMessage>> {
        let mut queue = self.message_queue.write().await;
        Ok(queue.pop())
    }

    async fn process_message(
        &self,
        msg: CollaborationMessage,
    ) -> GlobalResult<CollaborationResult> {
        let start = std::time::Instant::now();

        tracing::debug!(
            "[Parallel] {} processing message from {}",
            self.helper.agent_id(),
            msg.sender
        );

        let content = if self.helper.use_llm {
            self.helper
                .process_with_llm(
                    &msg,
                    "你是一个协作 Agent，负责处理并行模式的协作消息。任务会被分解并并行处理。",
                    // "You are a collaboration Agent handling Parallel messages. Tasks will be decomposed and processed in parallel."
                )
                .await?
        } else {
            CollaborationContent::Text(format!(
                "已启动并行处理，最大工作线程: {}",
                // Parallel processing started, max workers: {}
                self.max_workers
            ))
        };

        let duration = start.elapsed().as_millis() as u64;

        Ok(
            CollaborationResult::success(content, duration, CollaborationMode::Parallel)
                .with_participant(self.helper.agent_id().to_string())
                .with_participant(msg.sender),
        )
    }

    fn stats(&self) -> HashMap<String, serde_json::Value> {
        HashMap::from([
            (
                "agent_id".to_string(),
                serde_json::json!(self.helper.agent_id()),
            ),
            (
                "use_llm".to_string(),
                serde_json::json!(self.helper.use_llm),
            ),
            (
                "max_workers".to_string(),
                serde_json::json!(self.max_workers),
            ),
        ])
    }
}

// ============================================================================
// 顺序协议
// Sequential Protocol
// ============================================================================

/// 顺序协作协议
/// Sequential collaboration protocol
pub struct SequentialProtocol {
    helper: LLMProtocolHelper,
    message_queue: Arc<RwLock<Vec<CollaborationMessage>>>,
}

impl SequentialProtocol {
    /// 创建新的顺序协议（不使用 LLM）
    /// Create new Sequential protocol (without LLM)
    pub fn new(agent_id: impl Into<String>) -> Self {
        Self {
            helper: LLMProtocolHelper::new(agent_id),
            message_queue: Arc::new(RwLock::new(Vec::new())),
        }
    }

    /// 创建支持 LLM 的顺序协议
    /// Create LLM-enabled Sequential protocol
    pub fn with_llm(agent_id: impl Into<String>, llm_client: Arc<LLMClient>) -> Self {
        Self {
            helper: LLMProtocolHelper::new(agent_id).with_llm(llm_client),
            message_queue: Arc::new(RwLock::new(Vec::new())),
        }
    }
}

#[async_trait]
impl CollaborationProtocol for SequentialProtocol {
    fn name(&self) -> &str {
        "sequential"
    }

    fn description(&self) -> &str {
        "顺序协作协议：串行执行有依赖的任务链，适合流水线处理"
        // Sequential protocol: Serial execution of dependent task chains, suitable for pipeline processing
    }

    fn applicable_scenarios(&self) -> Vec<String> {
        vec![
            "流水线处理".to_string(),
            // Pipeline processing
            "依赖任务链".to_string(),
            // Dependent task chains
            "分步执行".to_string(),
            // Step-by-step execution
            "阶段式工作流".to_string(),
            // Phased workflow
        ]
    }

    fn mode(&self) -> CollaborationMode {
        CollaborationMode::Sequential
    }

    async fn send_message(&self, msg: CollaborationMessage) -> GlobalResult<()> {
        let mut queue = self.message_queue.write().await;
        queue.push(msg);
        Ok(())
    }

    async fn receive_message(&self) -> GlobalResult<Option<CollaborationMessage>> {
        let mut queue = self.message_queue.write().await;
        Ok(queue.pop())
    }

    async fn process_message(
        &self,
        msg: CollaborationMessage,
    ) -> GlobalResult<CollaborationResult> {
        let start = std::time::Instant::now();

        tracing::debug!(
            "[Sequential] {} processing message from {}",
            self.helper.agent_id(),
            msg.sender
        );

        let content = if self.helper.use_llm {
            self.helper
                .process_with_llm(
                    &msg,
                    "你是一个协作 Agent，负责处理顺序模式的协作消息。任务会按依赖关系依次执行。",
                    // "You are a collaboration Agent handling Sequential messages. Tasks will be executed in order based on dependencies."
                )
                .await?
        } else {
            CollaborationContent::Text("已执行顺序任务链".to_string())
            // Sequential task chain executed
        };

        let duration = start.elapsed().as_millis() as u64;

        Ok(
            CollaborationResult::success(content, duration, CollaborationMode::Sequential)
                .with_participant(self.helper.agent_id().to_string())
                .with_participant(msg.sender),
        )
    }

    fn stats(&self) -> HashMap<String, serde_json::Value> {
        HashMap::from([
            (
                "agent_id".to_string(),
                serde_json::json!(self.helper.agent_id()),
            ),
            (
                "use_llm".to_string(),
                serde_json::json!(self.helper.use_llm),
            ),
        ])
    }
}
