//! LLM 驱动的协作协议实现
//!
//! 本模块提供 MoFA 框架的标准协作协议实现，**所有协议都可以选择性地使用 LLM**。
//!
//! # 核心理念
//!
//! 在 Agent 框架中，协作协议应该能够：
//! 1. 使用 LLM 来理解和处理自然语言消息
//! 2. 通过 LLM 进行智能决策
//! 3. 记录 LLM 的推理过程

// 导出类型定义
pub mod types;

// 重新导出核心类型
pub use types::*;

use crate::llm::LLMClient;
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// 获取协作模式的描述
fn get_mode_description(mode: &CollaborationMode) -> &'static str {
    match mode {
        CollaborationMode::RequestResponse => "一对一的请求-响应，适合确定性任务",
        CollaborationMode::PublishSubscribe => "一对多的发布订阅，适合发散性任务",
        CollaborationMode::Consensus => "多 Agent 共识决策，适合需要达成一致的场景",
        CollaborationMode::Debate => "多轮辩论优化，适合需要迭代改进的审查任务",
        CollaborationMode::Parallel => "并行处理独立任务，适合可分解的工作",
        CollaborationMode::Sequential => "顺序执行有依赖的任务链",
        CollaborationMode::Custom(_) => "自定义模式",
    }
}

// ============================================================================
// LLM 辅助的协作协议基类
// ============================================================================

/// LLM 辅助协议基类
///
/// 提供协议与 LLM 交互的基础设施
pub struct LLMProtocolHelper {
    /// Agent ID
    agent_id: String,
    /// LLM 客户端（可选）
    llm_client: Option<Arc<LLMClient>>,
    /// 是否使用 LLM 处理消息
    use_llm: bool,
}

impl LLMProtocolHelper {
    /// 创建新的 LLM 协议辅助器
    pub fn new(agent_id: impl Into<String>) -> Self {
        Self {
            agent_id: agent_id.into(),
            llm_client: None,
            use_llm: false,
        }
    }

    /// 设置 LLM 客户端
    pub fn with_llm(mut self, llm_client: Arc<LLMClient>) -> Self {
        self.llm_client = Some(llm_client);
        self.use_llm = true;
        self
    }

    /// 启用/禁用 LLM
    pub fn with_use_llm(mut self, use_llm: bool) -> Self {
        self.use_llm = use_llm;
        self
    }

    /// 获取 Agent ID
    pub fn agent_id(&self) -> &str {
        &self.agent_id
    }

    /// 使用 LLM 处理消息
    pub async fn process_with_llm(
        &self,
        msg: &CollaborationMessage,
        system_prompt: &str,
    ) -> anyhow::Result<CollaborationContent> {
        let llm_client = self
            .llm_client
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("LLM client not configured"))?;

        let user_prompt = format!(
            "你是 {}。收到一条协作消息：\n\n发送者: {}\n内容: {}\n\n请处理这条消息并返回响应。",
            self.agent_id,
            msg.sender,
            msg.content.to_text()
        );

        let response = llm_client
            .chat()
            .system(system_prompt)
            .user(&user_prompt)
            .send()
            .await?;

        Ok(CollaborationContent::LLMResponse {
            reasoning: "通过 LLM 分析和处理协作消息".to_string(),
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
// ============================================================================

/// 请求-响应协作协议
///
/// 可选择使用 LLM 来智能处理请求
pub struct RequestResponseProtocol {
    helper: LLMProtocolHelper,
    message_queue: Arc<RwLock<Vec<CollaborationMessage>>>,
}

impl RequestResponseProtocol {
    /// 创建新的请求-响应协议（不使用 LLM）
    pub fn new(agent_id: impl Into<String>) -> Self {
        Self {
            helper: LLMProtocolHelper::new(agent_id),
            message_queue: Arc::new(RwLock::new(Vec::new())),
        }
    }

    /// 创建支持 LLM 的请求-响应协议
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
    }

    fn applicable_scenarios(&self) -> Vec<String> {
        vec![
            "数据查询和处理".to_string(),
            "确定性任务执行".to_string(),
            "状态获取".to_string(),
            "简单问答".to_string(),
        ]
    }

    fn mode(&self) -> CollaborationMode {
        CollaborationMode::RequestResponse
    }

    async fn send_message(&self, msg: CollaborationMessage) -> anyhow::Result<()> {
        let mut queue = self.message_queue.write().await;
        queue.push(msg);
        Ok(())
    }

    async fn receive_message(&self) -> anyhow::Result<Option<CollaborationMessage>> {
        let mut queue = self.message_queue.write().await;
        Ok(queue.pop())
    }

    async fn process_message(
        &self,
        msg: CollaborationMessage,
    ) -> anyhow::Result<CollaborationResult> {
        let start = std::time::Instant::now();

        tracing::debug!(
            "[RequestResponse] {} processing message from {}",
            self.helper.agent_id(),
            msg.sender
        );

        // 如果配置了 LLM，使用 LLM 处理
        let content = if self.helper.use_llm {
            self.helper
                .process_with_llm(
                    &msg,
                    "你是一个协作 Agent，负责处理请求-响应模式的协作消息。请理解请求内容并提供准确的响应。",
                )
                .await?
        } else {
            CollaborationContent::Text(format!("已收到来自 {} 的请求并处理", msg.sender))
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
// ============================================================================

/// 发布-订阅协作协议
pub struct PublishSubscribeProtocol {
    helper: LLMProtocolHelper,
    message_queue: Arc<RwLock<Vec<CollaborationMessage>>>,
    subscribed_topics: Arc<RwLock<std::collections::HashSet<String>>>,
}

impl PublishSubscribeProtocol {
    /// 创建新的发布-订阅协议（不使用 LLM）
    pub fn new(agent_id: impl Into<String>) -> Self {
        Self {
            helper: LLMProtocolHelper::new(agent_id),
            message_queue: Arc::new(RwLock::new(Vec::new())),
            subscribed_topics: Arc::new(RwLock::new(std::collections::HashSet::new())),
        }
    }

    /// 创建支持 LLM 的发布-订阅协议
    pub fn with_llm(agent_id: impl Into<String>, llm_client: Arc<LLMClient>) -> Self {
        Self {
            helper: LLMProtocolHelper::new(agent_id).with_llm(llm_client),
            message_queue: Arc::new(RwLock::new(Vec::new())),
            subscribed_topics: Arc::new(RwLock::new(std::collections::HashSet::new())),
        }
    }

    /// 订阅主题
    pub async fn subscribe(&self, topic: String) -> anyhow::Result<()> {
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
    }

    fn applicable_scenarios(&self) -> Vec<String> {
        vec![
            "创意生成和发散".to_string(),
            "通知广播".to_string(),
            "事件传播".to_string(),
            "多人协作".to_string(),
        ]
    }

    fn mode(&self) -> CollaborationMode {
        CollaborationMode::PublishSubscribe
    }

    async fn send_message(&self, msg: CollaborationMessage) -> anyhow::Result<()> {
        let mut queue = self.message_queue.write().await;
        queue.push(msg);
        Ok(())
    }

    async fn receive_message(&self) -> anyhow::Result<Option<CollaborationMessage>> {
        let mut queue = self.message_queue.write().await;
        Ok(queue.pop())
    }

    async fn process_message(
        &self,
        msg: CollaborationMessage,
    ) -> anyhow::Result<CollaborationResult> {
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
                )
                .await?
        } else {
            CollaborationContent::Text(format!("已发布消息到主题 {:?}", msg.topic))
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
// ============================================================================

/// 共识协作协议
pub struct ConsensusProtocol {
    helper: LLMProtocolHelper,
    message_queue: Arc<RwLock<Vec<CollaborationMessage>>>,
    threshold: f32,
}

impl ConsensusProtocol {
    /// 创建新的共识协议（不使用 LLM）
    pub fn new(agent_id: impl Into<String>) -> Self {
        Self {
            helper: LLMProtocolHelper::new(agent_id),
            message_queue: Arc::new(RwLock::new(Vec::new())),
            threshold: 0.7,
        }
    }

    /// 创建支持 LLM 的共识协议
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
    }

    fn applicable_scenarios(&self) -> Vec<String> {
        vec![
            "决策制定".to_string(),
            "投票评估".to_string(),
            "方案选择".to_string(),
            "质量评审".to_string(),
        ]
    }

    fn mode(&self) -> CollaborationMode {
        CollaborationMode::Consensus
    }

    async fn send_message(&self, msg: CollaborationMessage) -> anyhow::Result<()> {
        let mut queue = self.message_queue.write().await;
        queue.push(msg);
        Ok(())
    }

    async fn receive_message(&self) -> anyhow::Result<Option<CollaborationMessage>> {
        let mut queue = self.message_queue.write().await;
        Ok(queue.pop())
    }

    async fn process_message(
        &self,
        msg: CollaborationMessage,
    ) -> anyhow::Result<CollaborationResult> {
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
                )
                .await?
        } else {
            CollaborationContent::Text(format!("已参与共识决策，阈值: {}", self.threshold))
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
// ============================================================================

/// 辩论协作协议
pub struct DebateProtocol {
    helper: LLMProtocolHelper,
    message_queue: Arc<RwLock<Vec<CollaborationMessage>>>,
    max_rounds: usize,
}

impl DebateProtocol {
    /// 创建新的辩论协议（不使用 LLM）
    pub fn new(agent_id: impl Into<String>) -> Self {
        Self {
            helper: LLMProtocolHelper::new(agent_id),
            message_queue: Arc::new(RwLock::new(Vec::new())),
            max_rounds: 3,
        }
    }

    /// 创建支持 LLM 的辩论协议
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
    }

    fn applicable_scenarios(&self) -> Vec<String> {
        vec![
            "代码审查".to_string(),
            "方案优化".to_string(),
            "争议解决".to_string(),
            "质量改进".to_string(),
        ]
    }

    fn mode(&self) -> CollaborationMode {
        CollaborationMode::Debate
    }

    async fn send_message(&self, msg: CollaborationMessage) -> anyhow::Result<()> {
        let mut queue = self.message_queue.write().await;
        queue.push(msg);
        Ok(())
    }

    async fn receive_message(&self) -> anyhow::Result<Option<CollaborationMessage>> {
        let mut queue = self.message_queue.write().await;
        Ok(queue.pop())
    }

    async fn process_message(
        &self,
        msg: CollaborationMessage,
    ) -> anyhow::Result<CollaborationResult> {
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
                )
                .await?
        } else {
            CollaborationContent::Text(format!("已参与辩论，最大轮数: {}", self.max_rounds))
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
// ============================================================================

/// 并行协作协议
pub struct ParallelProtocol {
    helper: LLMProtocolHelper,
    message_queue: Arc<RwLock<Vec<CollaborationMessage>>>,
    max_workers: usize,
}

impl ParallelProtocol {
    /// 创建新的并行协议（不使用 LLM）
    pub fn new(agent_id: impl Into<String>) -> Self {
        Self {
            helper: LLMProtocolHelper::new(agent_id),
            message_queue: Arc::new(RwLock::new(Vec::new())),
            max_workers: 4,
        }
    }

    /// 创建支持 LLM 的并行协议
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
    }

    fn applicable_scenarios(&self) -> Vec<String> {
        vec![
            "数据分析".to_string(),
            "批量处理".to_string(),
            "分布式搜索".to_string(),
            "并行计算".to_string(),
        ]
    }

    fn mode(&self) -> CollaborationMode {
        CollaborationMode::Parallel
    }

    async fn send_message(&self, msg: CollaborationMessage) -> anyhow::Result<()> {
        let mut queue = self.message_queue.write().await;
        queue.push(msg);
        Ok(())
    }

    async fn receive_message(&self) -> anyhow::Result<Option<CollaborationMessage>> {
        let mut queue = self.message_queue.write().await;
        Ok(queue.pop())
    }

    async fn process_message(
        &self,
        msg: CollaborationMessage,
    ) -> anyhow::Result<CollaborationResult> {
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
                )
                .await?
        } else {
            CollaborationContent::Text(format!(
                "已启动并行处理，最大工作线程: {}",
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
// ============================================================================

/// 顺序协作协议
pub struct SequentialProtocol {
    helper: LLMProtocolHelper,
    message_queue: Arc<RwLock<Vec<CollaborationMessage>>>,
}

impl SequentialProtocol {
    /// 创建新的顺序协议（不使用 LLM）
    pub fn new(agent_id: impl Into<String>) -> Self {
        Self {
            helper: LLMProtocolHelper::new(agent_id),
            message_queue: Arc::new(RwLock::new(Vec::new())),
        }
    }

    /// 创建支持 LLM 的顺序协议
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
    }

    fn applicable_scenarios(&self) -> Vec<String> {
        vec![
            "流水线处理".to_string(),
            "依赖任务链".to_string(),
            "分步执行".to_string(),
            "阶段式工作流".to_string(),
        ]
    }

    fn mode(&self) -> CollaborationMode {
        CollaborationMode::Sequential
    }

    async fn send_message(&self, msg: CollaborationMessage) -> anyhow::Result<()> {
        let mut queue = self.message_queue.write().await;
        queue.push(msg);
        Ok(())
    }

    async fn receive_message(&self) -> anyhow::Result<Option<CollaborationMessage>> {
        let mut queue = self.message_queue.write().await;
        Ok(queue.pop())
    }

    async fn process_message(
        &self,
        msg: CollaborationMessage,
    ) -> anyhow::Result<CollaborationResult> {
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
                )
                .await?
        } else {
            CollaborationContent::Text("已执行顺序任务链".to_string())
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
