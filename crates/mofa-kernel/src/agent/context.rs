//! Agent 上下文定义
//! Agent Context Definition
//!
//! 统一的执行上下文，用于在 Agent 及其组件间传递状态
//! A unified execution context used to pass state between Agents and their components
//!
//! # 核心原则
//! # Core Principles
//!
//! CoreAgentContext 只包含内核原语（kernel primitives）：
//! CoreAgentContext only contains kernel primitives:
//! - 基本的状态存储（K/V store）
//! - Basic state storage (K/V store)
//! - 中断信号
//! - Interrupt signals
//! - 事件总线
//! - Event bus
//! - 配置
//! - Configuration
//! - 父子上下文关系
//! - Parent-child context relationships
//!
//! 业务逻辑（如指标收集、输出记录）应该在 foundation 层的 RichAgentContext 中实现。
//! Business logic (e.g., metrics collection, output logging) should be implemented in RichAgentContext at the foundation layer.

use serde::{Serialize, de::DeserializeOwned};
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use tokio::sync::{RwLock, mpsc};

// ============================================================================
// Agent 上下文
// Agent Context
// ============================================================================

/// 核心执行上下文 (Core Agent Context)
/// Core Execution Context (Core Agent Context)
///
/// 提供最小的内核原语用于 Agent 执行：
/// Provides minimal kernel primitives for Agent execution:
/// - 执行 ID 和会话 ID
/// - Execution ID and Session ID
/// - 父子上下文关系（用于嵌套执行）
/// - Parent-child context relationships (for nested execution)
/// - 通用键值存储
/// - General key-value storage
/// - 中断信号
/// - Interrupt signals
/// - 事件总线
/// - Event bus
/// - 配置
/// - Configuration
///
/// # 示例
/// # Example
///
/// ```rust,ignore
/// use mofa_kernel::agent::context::CoreAgentContext;
///
/// let ctx = CoreAgentContext::new("execution-123");
/// ctx.set("user_id", "user-456").await;
/// let value: Option<String> = ctx.get("user_id").await;
/// ```
#[derive(Clone)]
pub struct AgentContext<S: Clone = serde_json::Value> {
    /// 执行 ID (唯一标识本次执行)
    /// Execution ID (unique identifier for this execution)
    pub execution_id: String,
    /// 会话 ID (用于多轮对话)
    /// Session ID (used for multi-turn conversations)
    pub session_id: Option<String>,
    /// 父上下文 (用于层级执行)
    /// Parent context (used for hierarchical execution)
    parent: Option<Arc<AgentContext<S>>>,
    /// 共享状态 (通用键值存储)
    /// Shared state (general key-value storage)
    state: Arc<RwLock<HashMap<String, S>>>,
    /// 中断信号
    /// Interrupt signal
    interrupt: Arc<InterruptSignal>,
    /// 事件总线
    /// Event bus
    event_bus: Arc<EventBus<S>>,
    /// 配置
    /// Configuration
    config: Arc<ContextConfig<S>>,
}

/// 上下文配置
/// Context Configuration
#[derive(Debug, Clone)]
pub struct ContextConfig<S = serde_json::Value> {
    /// 超时时间 (毫秒)
    /// Timeout duration (milliseconds)
    pub timeout_ms: Option<u64>,
    /// 最大重试次数
    /// Maximum retry attempts
    pub max_retries: u32,
    /// 是否启用追踪
    /// Whether to enable tracing
    pub enable_tracing: bool,
    /// 自定义配置
    /// Custom configuration
    pub custom: HashMap<String, S>,
}

impl<S> Default for ContextConfig<S> {
    fn default() -> Self {
        Self {
            timeout_ms: None,
            max_retries: 3,
            enable_tracing: false,
            custom: HashMap::new(),
        }
    }
}

impl<S> AgentContext<S>
where
    S: Clone + Send + Sync + 'static,
{
    /// 创建新的上下文
    /// Create a new context
    pub fn new(execution_id: impl Into<String>) -> Self {
        Self {
            execution_id: execution_id.into(),
            session_id: None,
            parent: None,
            state: Arc::new(RwLock::new(HashMap::new())),
            interrupt: Arc::new(InterruptSignal::new()),
            event_bus: Arc::new(EventBus::new()),
            config: Arc::new(ContextConfig::default()),
        }
    }

    /// 创建带会话 ID 的上下文
    /// Create a context with a session ID
    pub fn with_session(execution_id: impl Into<String>, session_id: impl Into<String>) -> Self {
        let mut ctx = Self::new(execution_id);
        ctx.session_id = Some(session_id.into());
        ctx
    }

    /// 创建子上下文 (用于子任务执行)
    /// Create a child context (for sub-task execution)
    pub fn child(&self, execution_id: impl Into<String>) -> Self {
        Self {
            execution_id: execution_id.into(),
            session_id: self.session_id.clone(),
            parent: Some(Arc::new(self.clone())),
            state: Arc::new(RwLock::new(HashMap::new())),
            interrupt: self.interrupt.clone(), // 共享中断信号
            // Shared interrupt signal
            event_bus: self.event_bus.clone(), // 共享事件总线
            // Shared event bus
            config: self.config.clone(),
        }
    }

    /// 设置配置
    /// Set configuration
    pub fn with_config(mut self, config: ContextConfig<S>) -> Self {
        self.config = Arc::new(config);
        self
    }

    /// 获取值
    /// Get a value
    pub async fn get(&self, key: &str) -> Option<S> {
        let state = self.state.read().await;
        state.get(key).cloned()
    }

    /// 设置值
    /// Set a value
    pub async fn set(&self, key: &str, value: S) {
        let mut state = self.state.write().await;
        state.insert(key.to_string(), value);
    }

    /// 删除值
    /// Remove a value
    pub async fn remove(&self, key: &str) -> Option<S> {
        let mut state = self.state.write().await;
        state.remove(key)
    }

    /// 检查是否存在值
    /// Check if a value exists
    pub async fn contains(&self, key: &str) -> bool {
        let state = self.state.read().await;
        state.contains_key(key)
    }

    /// 获取所有键
    /// Get all keys
    pub async fn keys(&self) -> Vec<String> {
        let state = self.state.read().await;
        state.keys().cloned().collect()
    }

    /// 检查是否被中断
    /// Check if interrupted
    pub fn is_interrupted(&self) -> bool {
        self.interrupt.is_triggered()
    }

    /// 触发中断
    /// Trigger an interrupt
    pub fn trigger_interrupt(&self) {
        self.interrupt.trigger();
    }

    /// 清除中断状态
    /// Clear interrupt status
    pub fn clear_interrupt(&self) {
        self.interrupt.clear();
    }

    /// 获取配置
    /// Get configuration
    pub fn config(&self) -> &ContextConfig<S> {
        &self.config
    }

    /// 获取父上下文
    /// Get parent context
    pub fn parent(&self) -> Option<&Arc<AgentContext<S>>> {
        self.parent.as_ref()
    }

    /// 发送事件
    /// Emit an event
    pub async fn emit_event(&self, event: AgentEvent<S>) {
        self.event_bus.emit(event).await;
    }

    /// 订阅事件
    /// Subscribe to events
    pub async fn subscribe(&self, event_type: &str) -> EventReceiver<S> {
        self.event_bus.subscribe(event_type).await
    }

    /// 从父上下文查找值 (递归向上查找)
    /// Find value from parent context (recursive lookup)
    pub async fn find(&self, key: &str) -> Option<S> {
        // 先在当前上下文查找
        // Check current context first
        if let Some(value) = self.get(key).await {
            return Some(value);
        }

        // 递归查找父上下文
        // Recursively look up parent context
        if let Some(parent) = &self.parent {
            return Box::pin(parent.find(key)).await;
        }

        None
    }
}

// ============================================================================
// 中断信号
// Interrupt Signal
// ============================================================================

/// 中断信号
/// Interrupt Signal
pub struct InterruptSignal {
    triggered: AtomicBool,
}

impl InterruptSignal {
    /// 创建新的中断信号
    /// Create a new interrupt signal
    pub fn new() -> Self {
        Self {
            triggered: AtomicBool::new(false),
        }
    }

    /// 检查是否已触发
    /// Check if already triggered
    pub fn is_triggered(&self) -> bool {
        self.triggered.load(Ordering::SeqCst)
    }

    /// 触发中断
    /// Trigger the interrupt
    pub fn trigger(&self) {
        self.triggered.store(true, Ordering::SeqCst);
    }

    /// 清除中断状态
    /// Clear the interrupt status
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
// Event Bus
// ============================================================================

/// Agent 事件
/// Agent Event
#[derive(Debug, Clone)]
pub struct AgentEvent<S = serde_json::Value> {
    /// 事件类型
    /// Event type
    pub event_type: String,
    /// 事件数据
    /// Event data
    pub data: S,
    /// 时间戳
    /// Timestamp
    pub timestamp_ms: u64,
    /// 来源
    /// Source
    pub source: Option<String>,
}

impl<S> AgentEvent<S> {
    /// Create a new event
    pub fn new(event_type: impl Into<String>, data: S) -> Self {
        let now = crate::utils::now_ms();

        Self {
            event_type: event_type.into(),
            data,
            timestamp_ms: now,
            source: None,
        }
    }

    /// 设置来源
    /// Set source
    pub fn with_source(mut self, source: impl Into<String>) -> Self {
        self.source = Some(source.into());
        self
    }
}

/// 事件接收器
/// Event Receiver
pub type EventReceiver<S = serde_json::Value> = tokio::sync::mpsc::Receiver<AgentEvent<S>>;

/// 事件总线
/// Event Bus
pub struct EventBus<S = serde_json::Value> {
    subscribers: RwLock<HashMap<String, Vec<mpsc::Sender<AgentEvent<S>>>>>,
}

impl<S: Send + Sync + 'static + Clone> EventBus<S> {
    /// 创建新的事件总线
    /// Create a new event bus
    pub fn new() -> Self {
        Self {
            subscribers: RwLock::new(HashMap::new()),
        }
    }

    /// 发送事件
    /// Emit an event
    pub async fn emit(&self, event: AgentEvent<S>) {
        let subscribers = self.subscribers.read().await;

        // 发送给类型特定订阅者
        // Send to type-specific subscribers
        if let Some(senders) = subscribers.get(&event.event_type) {
            for sender in senders {
                let _ = sender.send(event.clone()).await;
            }
        }

        // 发送给通配订阅者
        // Send to wildcard subscribers
        if let Some(senders) = subscribers.get("*") {
            for sender in senders {
                let _ = sender.send(event.clone()).await;
            }
        }
    }

    /// 订阅事件
    /// Subscribe to events
    pub async fn subscribe(&self, event_type: &str) -> EventReceiver<S> {
        let (tx, rx) = mpsc::channel::<AgentEvent<S>>(100);
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
        /// 测试上下文基本功能
        /// Test basic context functionality
        let ctx = AgentContext::<String>::new("test-execution");

        ctx.set("key1", "value1".to_string()).await;
        let value: Option<String> = ctx.get("key1").await;
        assert_eq!(value, Some("value1".to_string()));
    }

    #[tokio::test]
    async fn test_context_child() {
        /// 测试子上下文
        /// Test child context
        let parent = AgentContext::<String>::new("parent");
        parent.set("parent_key", "parent_value".to_string()).await;

        let child = parent.child("child");
        child.set("child_key", "child_value".to_string()).await;

        // 子上下文可以访问自己的值
        // Child context can access its own values
        let child_value: Option<String> = child.get("child_key").await;
        assert_eq!(child_value, Some("child_value".to_string()));

        // 子上下文不能直接访问父上下文的值 (需要用 find)
        // Child context cannot access parent values directly (requires 'find')
        let parent_value: Option<String> = child.find("parent_key").await;
        assert_eq!(parent_value, Some("parent_value".to_string()));
    }

    #[tokio::test]
    async fn test_interrupt_signal() {
        /// 测试中断信号
        /// Test interrupt signal
        let ctx = AgentContext::<String>::new("test");

        assert!(!ctx.is_interrupted());
        ctx.trigger_interrupt();
        assert!(ctx.is_interrupted());
        ctx.clear_interrupt();
        assert!(!ctx.is_interrupted());
    }

    #[tokio::test]
    async fn test_event_bus() {
        /// 测试事件总线
        /// Test event bus
        let ctx = AgentContext::<String>::new("test");

        let mut rx = ctx.subscribe("test_event").await;

        ctx.emit_event(AgentEvent::<String>::new(
            "test_event",
            "hello".to_string(),
        ))
        .await;

        let event = rx.recv().await.unwrap();
        assert_eq!(event.event_type, "test_event");
    }
}
