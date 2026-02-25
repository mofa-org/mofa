//! Rich Agent Context - 扩展上下文
//! Rich Agent Context - Extended Context
//!
//! 提供业务特定的功能，扩展内核的 CoreAgentContext
//! Provides business-specific functions to extend the kernel's CoreAgentContext

use mofa_kernel::agent::context::AgentContext;
use serde::{Serialize, de::DeserializeOwned};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// 组件输出记录
/// Component output record
#[derive(Debug, Clone)]
pub struct ComponentOutput {
    /// 组件名称
    /// Component name
    pub component: String,
    /// 输出内容
    /// Output content
    pub output: serde_json::Value,
    /// 时间戳
    /// Timestamp
    pub timestamp_ms: u64,
}

/// 执行指标
/// Execution metrics
#[derive(Debug, Clone, Default)]
pub struct ExecutionMetrics {
    /// 开始时间
    /// Start time
    pub start_time_ms: u64,
    /// 结束时间
    /// End time
    pub end_time_ms: Option<u64>,
    /// 组件执行次数
    /// Component execution count
    pub component_calls: HashMap<String, u64>,
    /// Token 使用
    /// Token usage
    pub total_tokens: u64,
    /// 工具调用次数
    /// Tool call count
    pub tool_calls: u64,
}

impl ExecutionMetrics {
    /// 创建新的指标
    /// Create new metrics
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
    /// Get execution duration (ms)
    pub fn duration_ms(&self) -> u64 {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;

        self.end_time_ms.unwrap_or(now) - self.start_time_ms
    }
}

/// 扩展的 Agent 上下文
/// Extended Agent Context
///
/// 提供业务特定的功能：
/// Provides business-specific functions:
/// - 组件输出记录
/// - Component output recording
/// - 执行指标跟踪
/// - Execution metrics tracking
/// - 委托所有核心功能到 CoreAgentContext
/// - Delegate all core functions to CoreAgentContext
///
/// # 示例
/// # Example
///
/// ```rust,ignore
/// use mofa_foundation::agent::context::RichAgentContext;
/// use mofa_kernel::agent::context::CoreAgentContext;
///
/// let core_ctx = CoreAgentContext::new("execution-123");
/// let rich_ctx = RichAgentContext::from(core_ctx);
///
/// // 业务特定功能
/// // Business specific functions
/// rich_ctx.record_output("llm", serde_json::json!("response")).await;
/// rich_ctx.increment_component_calls("llm").await;
///
/// // 核心功能委托
/// // Core function delegation
/// rich_ctx.set("key", "value").await;
/// ```
#[derive(Clone)]
pub struct RichAgentContext {
    /// 内核上下文 (委托核心功能)
    /// Kernel context (delegates core functions)
    inner: Arc<AgentContext>,
    /// 累积输出
    /// Accumulated outputs
    outputs: Arc<RwLock<Vec<ComponentOutput>>>,
    /// 执行指标
    /// Execution metrics
    metrics: Arc<RwLock<ExecutionMetrics>>,
}

impl RichAgentContext {
    /// 从 CoreAgentContext 创建 RichAgentContext
    /// Create RichAgentContext from CoreAgentContext
    pub fn new(inner: AgentContext) -> Self {
        Self {
            inner: Arc::new(inner),
            outputs: Arc::new(RwLock::new(Vec::new())),
            metrics: Arc::new(RwLock::new(ExecutionMetrics::new())),
        }
    }

    /// 记录组件输出
    /// Record component output
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
    /// Get all component outputs
    pub async fn get_outputs(&self) -> Vec<ComponentOutput> {
        let outputs = self.outputs.read().await;
        outputs.clone()
    }

    /// 增加组件调用计数
    /// Increment component call count
    pub async fn increment_component_calls(&self, component: &str) {
        let mut metrics = self.metrics.write().await;
        *metrics
            .component_calls
            .entry(component.to_string())
            .or_insert(0) += 1;
    }

    /// 增加 Token 使用
    /// Add token usage
    pub async fn add_tokens(&self, tokens: u64) {
        let mut metrics = self.metrics.write().await;
        metrics.total_tokens += tokens;
    }

    /// 增加工具调用计数
    /// Increment tool call count
    pub async fn increment_tool_calls(&self) {
        let mut metrics = self.metrics.write().await;
        metrics.tool_calls += 1;
    }

    /// 获取执行指标
    /// Get execution metrics
    pub async fn get_metrics(&self) -> ExecutionMetrics {
        let metrics = self.metrics.read().await;
        metrics.clone()
    }

    /// 结束执行 (记录结束时间)
    /// Finish execution (record end time)
    pub async fn finish(&self) {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;

        let mut metrics = self.metrics.write().await;
        metrics.end_time_ms = Some(now);
    }

    /// 获取执行时长 (毫秒)
    /// Get execution duration (ms)
    pub async fn duration_ms(&self) -> u64 {
        let metrics = self.metrics.read().await;
        metrics.duration_ms()
    }

    // ===== 核心功能委托 =====
    // ===== Core Function Delegation =====

    /// 获取值
    /// Get value
    pub async fn get<T: DeserializeOwned>(&self, key: &str) -> Option<T> {
        self.inner.get(key).await.and_then(|v| {
            match serde_json::from_value(v) {
                Ok(val) => Some(val),
                Err(e) => {
                    tracing::warn!(key = key, error = %e, "RichAgentContext::get deserialization failed");
                    None
                }
            }
        })
    }

    /// 设置值
    /// Set value
    pub async fn set<T: Serialize>(&self, key: &str, value: T) {
        match serde_json::to_value(value) {
            Ok(v) => self.inner.set(key, v).await,
            Err(e) => tracing::warn!(key = key, error = %e, "RichAgentContext::set serialization failed"),
        }
    }

    /// 删除值
    /// Remove value
    pub async fn remove(&self, key: &str) -> Option<serde_json::Value> {
        self.inner.remove(key).await
    }

    /// 检查是否存在值
    /// Check if value exists
    pub async fn contains(&self, key: &str) -> bool {
        self.inner.contains(key).await
    }

    /// 获取所有键
    /// Get all keys
    pub async fn keys(&self) -> Vec<String> {
        self.inner.keys().await
    }

    /// 从父上下文查找值
    /// Find value from parent context
    pub async fn find<T: DeserializeOwned>(&self, key: &str) -> Option<T> {
        self.inner.find(key).await.and_then(|v| {
            match serde_json::from_value(v) {
                Ok(val) => Some(val),
                Err(e) => {
                    tracing::warn!(key = key, error = %e, "RichAgentContext::find deserialization failed");
                    None
                }
            }
        })
    }

    /// 获取执行 ID
    /// Get execution ID
    pub fn execution_id(&self) -> &str {
        &self.inner.execution_id
    }

    /// 获取会话 ID
    /// Get session ID
    pub fn session_id(&self) -> Option<&str> {
        self.inner.session_id.as_deref()
    }

    /// 获取父上下文
    /// Get parent context
    pub fn parent(&self) -> Option<&Arc<AgentContext>> {
        self.inner.parent()
    }

    /// 检查是否被中断
    /// Check if interrupted
    pub fn is_interrupted(&self) -> bool {
        self.inner.is_interrupted()
    }

    /// 触发中断
    /// Trigger interrupt
    pub fn trigger_interrupt(&self) {
        self.inner.trigger_interrupt()
    }

    /// 清除中断状态
    /// Clear interrupt status
    pub fn clear_interrupt(&self) {
        self.inner.clear_interrupt()
    }

    /// 获取配置
    /// Get configuration
    pub fn config(&self) -> &mofa_kernel::agent::context::ContextConfig {
        self.inner.config()
    }

    /// 发送事件
    /// Emit event
    pub async fn emit_event(&self, event: mofa_kernel::agent::context::AgentEvent) {
        self.inner.emit_event(event).await
    }

    /// 订阅事件
    /// Subscribe to events
    pub async fn subscribe(
        &self,
        event_type: &str,
    ) -> tokio::sync::mpsc::Receiver<mofa_kernel::agent::context::AgentEvent> {
        self.inner.subscribe(event_type).await
    }

    /// 获取内部核心上下文的引用
    /// Get reference to inner core context
    pub fn inner(&self) -> &AgentContext {
        &self.inner
    }
}

// ===== 转换实现 =====
// ===== Conversion Implementation =====

impl From<AgentContext> for RichAgentContext {
    fn from(inner: AgentContext) -> Self {
        Self::new(inner)
    }
}

impl From<RichAgentContext> for AgentContext {
    fn from(rich: RichAgentContext) -> Self {
        // 注意：这会克隆内部上下文，丢失 RichAgentContext 的扩展状态
        // Note: This clones the inner context, losing extended state of RichAgentContext
        // 在实际使用中，应该通过 AsRef trait 来获取引用
        // In actual usage, reference should be obtained via AsRef trait
        (*rich.inner).clone()
    }
}

impl AsRef<AgentContext> for RichAgentContext {
    fn as_ref(&self) -> &AgentContext {
        &self.inner
    }
}
