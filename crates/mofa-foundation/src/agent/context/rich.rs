//! Rich Agent Context - 扩展上下文
//!
//! 提供业务特定的功能，扩展内核的 CoreAgentContext

use mofa_kernel::agent::context::AgentContext;
use serde::{Serialize, de::DeserializeOwned};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// 组件输出记录
#[derive(Debug, Clone)]
pub struct ComponentOutput {
    /// 组件名称
    pub component: String,
    /// 输出内容
    pub output: serde_json::Value,
    /// 时间戳
    pub timestamp_ms: u64,
}

/// 执行指标
#[derive(Debug, Clone, Default)]
pub struct ExecutionMetrics {
    /// 开始时间
    pub start_time_ms: u64,
    /// 结束时间
    pub end_time_ms: Option<u64>,
    /// 组件执行次数
    pub component_calls: HashMap<String, u64>,
    /// Token 使用
    pub total_tokens: u64,
    /// 工具调用次数
    pub tool_calls: u64,
}

impl ExecutionMetrics {
    /// 创建新的指标
    pub fn new() -> Self {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;

        Self {
            start_time_ms: now,
            ..Default::default()
        }
    }

    /// 获取执行时长 (毫秒)
    pub fn duration_ms(&self) -> u64 {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;

        self.end_time_ms.unwrap_or(now) - self.start_time_ms
    }
}

/// 扩展的 Agent 上下文
///
/// 提供业务特定的功能：
/// - 组件输出记录
/// - 执行指标跟踪
/// - 委托所有核心功能到 CoreAgentContext
///
/// # 示例
///
/// ```rust,ignore
/// use mofa_foundation::agent::context::RichAgentContext;
/// use mofa_kernel::agent::context::CoreAgentContext;
///
/// let core_ctx = CoreAgentContext::new("execution-123");
/// let rich_ctx = RichAgentContext::from(core_ctx);
///
/// // 业务特定功能
/// rich_ctx.record_output("llm", serde_json::json!("response")).await;
/// rich_ctx.increment_component_calls("llm").await;
///
/// // 核心功能委托
/// rich_ctx.set("key", "value").await;
/// ```
#[derive(Clone)]
pub struct RichAgentContext {
    /// 内核上下文 (委托核心功能)
    inner: Arc<AgentContext>,
    /// 累积输出
    outputs: Arc<RwLock<Vec<ComponentOutput>>>,
    /// 执行指标
    metrics: Arc<RwLock<ExecutionMetrics>>,
}

impl RichAgentContext {
    /// 从 CoreAgentContext 创建 RichAgentContext
    pub fn new(inner: AgentContext) -> Self {
        Self {
            inner: Arc::new(inner),
            outputs: Arc::new(RwLock::new(Vec::new())),
            metrics: Arc::new(RwLock::new(ExecutionMetrics::new())),
        }
    }

    /// 记录组件输出
    pub async fn record_output(&self, component: impl Into<String>, output: serde_json::Value) {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;

        let mut outputs = self.outputs.write().await;
        outputs.push(ComponentOutput {
            component: component.into(),
            output,
            timestamp_ms: now,
        });
    }

    /// 获取所有组件输出
    pub async fn get_outputs(&self) -> Vec<ComponentOutput> {
        let outputs = self.outputs.read().await;
        outputs.clone()
    }

    /// 增加组件调用计数
    pub async fn increment_component_calls(&self, component: &str) {
        let mut metrics = self.metrics.write().await;
        *metrics
            .component_calls
            .entry(component.to_string())
            .or_insert(0) += 1;
    }

    /// 增加 Token 使用
    pub async fn add_tokens(&self, tokens: u64) {
        let mut metrics = self.metrics.write().await;
        metrics.total_tokens += tokens;
    }

    /// 增加工具调用计数
    pub async fn increment_tool_calls(&self) {
        let mut metrics = self.metrics.write().await;
        metrics.tool_calls += 1;
    }

    /// 获取执行指标
    pub async fn get_metrics(&self) -> ExecutionMetrics {
        let metrics = self.metrics.read().await;
        metrics.clone()
    }

    /// 结束执行 (记录结束时间)
    pub async fn finish(&self) {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;

        let mut metrics = self.metrics.write().await;
        metrics.end_time_ms = Some(now);
    }

    /// 获取执行时长 (毫秒)
    pub async fn duration_ms(&self) -> u64 {
        let metrics = self.metrics.read().await;
        metrics.duration_ms()
    }

    // ===== 核心功能委托 =====

    /// 获取值
    pub async fn get<T: DeserializeOwned>(&self, key: &str) -> Option<T> {
        self.inner.get(key).await
    }

    /// 设置值
    pub async fn set<T: Serialize>(&self, key: &str, value: T) {
        self.inner.set(key, value).await
    }

    /// 删除值
    pub async fn remove(&self, key: &str) -> Option<serde_json::Value> {
        self.inner.remove(key).await
    }

    /// 检查是否存在值
    pub async fn contains(&self, key: &str) -> bool {
        self.inner.contains(key).await
    }

    /// 获取所有键
    pub async fn keys(&self) -> Vec<String> {
        self.inner.keys().await
    }

    /// 从父上下文查找值
    pub async fn find<T: DeserializeOwned>(&self, key: &str) -> Option<T> {
        self.inner.find(key).await
    }

    /// 获取执行 ID
    pub fn execution_id(&self) -> &str {
        &self.inner.execution_id
    }

    /// 获取会话 ID
    pub fn session_id(&self) -> Option<&str> {
        self.inner.session_id.as_deref()
    }

    /// 获取父上下文
    pub fn parent(&self) -> Option<&Arc<AgentContext>> {
        self.inner.parent()
    }

    /// 检查是否被中断
    pub fn is_interrupted(&self) -> bool {
        self.inner.is_interrupted()
    }

    /// 触发中断
    pub fn trigger_interrupt(&self) {
        self.inner.trigger_interrupt()
    }

    /// 清除中断状态
    pub fn clear_interrupt(&self) {
        self.inner.clear_interrupt()
    }

    /// 获取配置
    pub fn config(&self) -> &mofa_kernel::agent::context::ContextConfig {
        self.inner.config()
    }

    /// 发送事件
    pub async fn emit_event(&self, event: mofa_kernel::agent::context::AgentEvent) {
        self.inner.emit_event(event).await
    }

    /// 订阅事件
    pub async fn subscribe(
        &self,
        event_type: &str,
    ) -> tokio::sync::mpsc::Receiver<mofa_kernel::agent::context::AgentEvent> {
        self.inner.subscribe(event_type).await
    }

    /// 获取内部核心上下文的引用
    pub fn inner(&self) -> &AgentContext {
        &self.inner
    }
}

// ===== 转换实现 =====

impl From<AgentContext> for RichAgentContext {
    fn from(inner: AgentContext) -> Self {
        Self::new(inner)
    }
}

impl From<RichAgentContext> for AgentContext {
    fn from(rich: RichAgentContext) -> Self {
        // 注意：这会克隆内部上下文，丢失 RichAgentContext 的扩展状态
        // 在实际使用中，应该通过 AsRef trait 来获取引用
        (*rich.inner).clone()
    }
}

impl AsRef<AgentContext> for RichAgentContext {
    fn as_ref(&self) -> &AgentContext {
        &self.inner
    }
}
