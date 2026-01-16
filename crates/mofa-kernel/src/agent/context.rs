//! Agent 上下文定义
//!
//! 统一的执行上下文，用于在 Agent 及其组件间传递状态

use serde::{de::DeserializeOwned, Serialize};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};

// ============================================================================
// Agent 上下文
// ============================================================================

/// Agent 执行上下文
///
/// 在 Agent 执行过程中传递状态、配置和信号
#[derive(Clone)]
pub struct AgentContext {
    /// 执行 ID (唯一标识本次执行)
    pub execution_id: String,
    /// 会话 ID (用于多轮对话)
    pub session_id: Option<String>,
    /// 父上下文 (用于层级执行)
    parent: Option<Arc<AgentContext>>,
    /// 共享状态
    state: Arc<RwLock<ContextState>>,
    /// 中断信号
    interrupt: Arc<InterruptSignal>,
    /// 事件总线
    event_bus: Arc<EventBus>,
    /// 配置
    config: Arc<ContextConfig>,
}

/// 上下文内部状态
#[derive(Default)]
struct ContextState {
    /// 键值存储
    values: HashMap<String, serde_json::Value>,
    /// 累积输出
    outputs: Vec<ComponentOutput>,
    /// 执行指标
    metrics: ExecutionMetrics,
}

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

/// 上下文配置
#[derive(Debug, Clone, Default)]
pub struct ContextConfig {
    /// 超时时间 (毫秒)
    pub timeout_ms: Option<u64>,
    /// 最大重试次数
    pub max_retries: u32,
    /// 是否启用追踪
    pub enable_tracing: bool,
    /// 自定义配置
    pub custom: HashMap<String, serde_json::Value>,
}

impl AgentContext {
    /// 创建新的上下文
    pub fn new(execution_id: impl Into<String>) -> Self {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;

        let mut state = ContextState::default();
        state.metrics.start_time_ms = now;

        Self {
            execution_id: execution_id.into(),
            session_id: None,
            parent: None,
            state: Arc::new(RwLock::new(state)),
            interrupt: Arc::new(InterruptSignal::new()),
            event_bus: Arc::new(EventBus::new()),
            config: Arc::new(ContextConfig::default()),
        }
    }

    /// 创建带会话 ID 的上下文
    pub fn with_session(execution_id: impl Into<String>, session_id: impl Into<String>) -> Self {
        let mut ctx = Self::new(execution_id);
        ctx.session_id = Some(session_id.into());
        ctx
    }

    /// 创建子上下文 (用于子任务执行)
    pub fn child(&self, execution_id: impl Into<String>) -> Self {
        Self {
            execution_id: execution_id.into(),
            session_id: self.session_id.clone(),
            parent: Some(Arc::new(self.clone())),
            state: Arc::new(RwLock::new(ContextState::default())),
            interrupt: self.interrupt.clone(), // 共享中断信号
            event_bus: self.event_bus.clone(), // 共享事件总线
            config: self.config.clone(),
        }
    }

    /// 设置配置
    pub fn with_config(mut self, config: ContextConfig) -> Self {
        self.config = Arc::new(config);
        self
    }

    /// 获取值
    pub async fn get<T: DeserializeOwned>(&self, key: &str) -> Option<T> {
        let state = self.state.read().await;
        state
            .values
            .get(key)
            .and_then(|v| serde_json::from_value(v.clone()).ok())
    }

    /// 设置值
    pub async fn set<T: Serialize>(&self, key: &str, value: T) {
        if let Ok(v) = serde_json::to_value(value) {
            let mut state = self.state.write().await;
            state.values.insert(key.to_string(), v);
        }
    }

    /// 删除值
    pub async fn remove(&self, key: &str) -> Option<serde_json::Value> {
        let mut state = self.state.write().await;
        state.values.remove(key)
    }

    /// 检查是否存在值
    pub async fn contains(&self, key: &str) -> bool {
        let state = self.state.read().await;
        state.values.contains_key(key)
    }

    /// 获取所有键
    pub async fn keys(&self) -> Vec<String> {
        let state = self.state.read().await;
        state.values.keys().cloned().collect()
    }

    /// 记录组件输出
    pub async fn record_output(&self, component: impl Into<String>, output: serde_json::Value) {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;

        let mut state = self.state.write().await;
        state.outputs.push(ComponentOutput {
            component: component.into(),
            output,
            timestamp_ms: now,
        });
    }

    /// 获取所有组件输出
    pub async fn get_outputs(&self) -> Vec<ComponentOutput> {
        let state = self.state.read().await;
        state.outputs.clone()
    }

    /// 增加组件调用计数
    pub async fn increment_component_calls(&self, component: &str) {
        let mut state = self.state.write().await;
        *state.metrics.component_calls.entry(component.to_string()).or_insert(0) += 1;
    }

    /// 增加 Token 使用
    pub async fn add_tokens(&self, tokens: u64) {
        let mut state = self.state.write().await;
        state.metrics.total_tokens += tokens;
    }

    /// 增加工具调用计数
    pub async fn increment_tool_calls(&self) {
        let mut state = self.state.write().await;
        state.metrics.tool_calls += 1;
    }

    /// 获取执行指标
    pub async fn get_metrics(&self) -> ExecutionMetrics {
        let state = self.state.read().await;
        state.metrics.clone()
    }

    /// 结束执行 (记录结束时间)
    pub async fn finish(&self) {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;

        let mut state = self.state.write().await;
        state.metrics.end_time_ms = Some(now);
    }

    /// 获取执行时长 (毫秒)
    pub async fn duration_ms(&self) -> u64 {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;

        let state = self.state.read().await;
        state.metrics.end_time_ms.unwrap_or(now) - state.metrics.start_time_ms
    }

    /// 检查是否被中断
    pub fn is_interrupted(&self) -> bool {
        self.interrupt.is_triggered()
    }

    /// 触发中断
    pub fn trigger_interrupt(&self) {
        self.interrupt.trigger();
    }

    /// 清除中断状态
    pub fn clear_interrupt(&self) {
        self.interrupt.clear();
    }

    /// 获取配置
    pub fn config(&self) -> &ContextConfig {
        &self.config
    }

    /// 获取父上下文
    pub fn parent(&self) -> Option<&Arc<AgentContext>> {
        self.parent.as_ref()
    }

    /// 发送事件
    pub async fn emit_event(&self, event: AgentEvent) {
        self.event_bus.emit(event).await;
    }

    /// 订阅事件
    pub async fn subscribe(&self, event_type: &str) -> EventReceiver {
        self.event_bus.subscribe(event_type).await
    }

    /// 从父上下文查找值 (递归向上查找)
    pub async fn find<T: DeserializeOwned>(&self, key: &str) -> Option<T> {
        // 先在当前上下文查找
        if let Some(value) = self.get::<T>(key).await {
            return Some(value);
        }

        // 递归查找父上下文
        if let Some(parent) = &self.parent {
            return Box::pin(parent.find::<T>(key)).await;
        }

        None
    }
}

// ============================================================================
// 中断信号
// ============================================================================

/// 中断信号
pub struct InterruptSignal {
    triggered: AtomicBool,
}

impl InterruptSignal {
    /// 创建新的中断信号
    pub fn new() -> Self {
        Self {
            triggered: AtomicBool::new(false),
        }
    }

    /// 检查是否已触发
    pub fn is_triggered(&self) -> bool {
        self.triggered.load(Ordering::SeqCst)
    }

    /// 触发中断
    pub fn trigger(&self) {
        self.triggered.store(true, Ordering::SeqCst);
    }

    /// 清除中断状态
    pub fn clear(&self) {
        self.triggered.store(false, Ordering::SeqCst);
    }
}

impl Default for InterruptSignal {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// 事件总线
// ============================================================================

/// Agent 事件
#[derive(Debug, Clone)]
pub struct AgentEvent {
    /// 事件类型
    pub event_type: String,
    /// 事件数据
    pub data: serde_json::Value,
    /// 时间戳
    pub timestamp_ms: u64,
    /// 来源
    pub source: Option<String>,
}

impl AgentEvent {
    /// 创建新事件
    pub fn new(event_type: impl Into<String>, data: serde_json::Value) -> Self {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;

        Self {
            event_type: event_type.into(),
            data,
            timestamp_ms: now,
            source: None,
        }
    }

    /// 设置来源
    pub fn with_source(mut self, source: impl Into<String>) -> Self {
        self.source = Some(source.into());
        self
    }
}

/// 事件接收器
pub type EventReceiver = mpsc::Receiver<AgentEvent>;

/// 事件总线
pub struct EventBus {
    subscribers: RwLock<HashMap<String, Vec<mpsc::Sender<AgentEvent>>>>,
}

impl EventBus {
    /// 创建新的事件总线
    pub fn new() -> Self {
        Self {
            subscribers: RwLock::new(HashMap::new()),
        }
    }

    /// 发送事件
    pub async fn emit(&self, event: AgentEvent) {
        let subscribers = self.subscribers.read().await;

        // 发送给类型特定订阅者
        if let Some(senders) = subscribers.get(&event.event_type) {
            for sender in senders {
                let _ = sender.send(event.clone()).await;
            }
        }

        // 发送给通配订阅者
        if let Some(senders) = subscribers.get("*") {
            for sender in senders {
                let _ = sender.send(event.clone()).await;
            }
        }
    }

    /// 订阅事件
    pub async fn subscribe(&self, event_type: &str) -> EventReceiver {
        let (tx, rx) = mpsc::channel(100);
        let mut subscribers = self.subscribers.write().await;
        subscribers
            .entry(event_type.to_string())
            .or_insert_with(Vec::new)
            .push(tx);
        rx
    }
}

impl Default for EventBus {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_context_basic() {
        let ctx = AgentContext::new("test-execution");

        ctx.set("key1", "value1").await;
        let value: Option<String> = ctx.get("key1").await;
        assert_eq!(value, Some("value1".to_string()));
    }

    #[tokio::test]
    async fn test_context_child() {
        let parent = AgentContext::new("parent");
        parent.set("parent_key", "parent_value").await;

        let child = parent.child("child");
        child.set("child_key", "child_value").await;

        // 子上下文可以访问自己的值
        let child_value: Option<String> = child.get("child_key").await;
        assert_eq!(child_value, Some("child_value".to_string()));

        // 子上下文不能直接访问父上下文的值 (需要用 find)
        let parent_value: Option<String> = child.find("parent_key").await;
        assert_eq!(parent_value, Some("parent_value".to_string()));
    }

    #[tokio::test]
    async fn test_interrupt_signal() {
        let ctx = AgentContext::new("test");

        assert!(!ctx.is_interrupted());
        ctx.trigger_interrupt();
        assert!(ctx.is_interrupted());
        ctx.clear_interrupt();
        assert!(!ctx.is_interrupted());
    }

    #[tokio::test]
    async fn test_event_bus() {
        let ctx = AgentContext::new("test");

        let mut rx = ctx.subscribe("test_event").await;

        ctx.emit_event(AgentEvent::new("test_event", serde_json::json!({"msg": "hello"}))).await;

        let event = rx.recv().await.unwrap();
        assert_eq!(event.event_type, "test_event");
    }

    #[tokio::test]
    async fn test_metrics() {
        let ctx = AgentContext::new("test");

        ctx.increment_component_calls("llm").await;
        ctx.increment_component_calls("llm").await;
        ctx.add_tokens(100).await;
        ctx.increment_tool_calls().await;

        let metrics = ctx.get_metrics().await;
        assert_eq!(metrics.component_calls.get("llm"), Some(&2));
        assert_eq!(metrics.total_tokens, 100);
        assert_eq!(metrics.tool_calls, 1);
    }
}
