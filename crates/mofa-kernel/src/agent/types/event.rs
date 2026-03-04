//! 全局事件系统
//! Global Event System
//!
//! 本模块提供全局事件系统，用于替代分散的 AgentEvent 和 PluginEvent 定义。
//! This module provides a global event system to replace scattered AgentEvent and PluginEvent definitions.
//!
//! # 设计目标
//! # Design Goals
//!
//! - 提供单一的事件类型，避免多处重复定义
//! - Provide a single event type to avoid redundant definitions
//! - 支持事件的发布-订阅模式
//! - Support publish-subscribe patterns for events
//! - 支持事件携带任意数据
//! - Support events carrying arbitrary data
//! - 提供清晰的事件来源和类型标识
//! - Provide clear identification of event sources and types

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ============================================================================
// GlobalEvent - 全局事件类型
// GlobalEvent - Global Event Type
// ============================================================================

/// 全局事件类型
/// Global event type
///
/// 替代 `AgentEvent` 和 `PluginEvent`，提供全局事件抽象。
/// Replaces `AgentEvent` and `PluginEvent`, providing a global event abstraction.
///
/// # 事件类型
/// # Event Types
///
/// 常见的事件类型包括：
/// Common event types include:
/// - `lifecycle:*` - 生命周期事件（created, initialized, started, stopped, shutdown）
/// - `lifecycle:*` - Lifecycle events (created, initialized, started, stopped, shutdown)
/// - `execution:*` - 执行事件（started, completed, failed, interrupted）
/// - `execution:*` - Execution events (started, completed, failed, interrupted)
/// - `message:*` - 消息事件（sent, received, delivered）
/// - `message:*` - Message events (sent, received, delivered)
/// - `plugin:*` - 插件事件（loaded, unloaded, error）
/// - `plugin:*` - Plugin events (loaded, unloaded, error)
/// - `state:*` - 状态变更事件（changed, error）
/// - `state:*` - State change events (changed, error)
/// - `custom:*` - 自定义事件
/// - `custom:*` - Custom events
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GlobalEvent {
    /// 事件 ID（唯一标识）
    /// Event ID (Unique Identifier)
    pub event_id: String,

    /// 事件类型
    /// Event Type
    ///
    /// 使用命名空间格式，如 "lifecycle:initialized"
    /// Uses namespace format, e.g., "lifecycle:initialized"
    pub event_type: String,

    /// 事件源（触发事件的 Agent 或组件 ID）
    /// Event Source (ID of the Agent or component triggering the event)
    pub source: String,

    /// 时间戳（毫秒）
    /// Timestamp (milliseconds)
    pub timestamp: u64,

    /// 事件数据（负载）
    /// Event Data (Payload)
    pub data: serde_json::Value,

    /// 元数据（额外属性）
    /// Metadata (Additional Attributes)
    pub metadata: HashMap<String, String>,
}

impl GlobalEvent {
    /// 创建新事件
    /// Create a new event
    pub fn new(event_type: impl Into<String>, source: impl Into<String>) -> Self {
        let timestamp = crate::utils::now_ms();

        Self {
            event_id: uuid::Uuid::new_v4().to_string(),
            event_type: event_type.into(),
            source: source.into(),
            timestamp,
            data: serde_json::Value::Null,
            metadata: HashMap::new(),
        }
    }

    /// 创建带数据的事件
    /// Create an event with data
    pub fn with_data(mut self, data: impl Into<serde_json::Value>) -> Self {
        self.data = data.into();
        self
    }

    /// 添加元数据
    /// Add metadata
    pub fn with_metadata(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.metadata.insert(key.into(), value.into());
        self
    }

    /// 获取特定数据字段
    /// Get specific data field
    pub fn get_data<T: for<'de> Deserialize<'de>>(&self) -> Option<T> {
        serde_json::from_value(self.data.clone()).ok()
    }

    /// 检查事件是否为指定类型
    /// Check if event is of specified type
    pub fn is_type(&self, event_type: &str) -> bool {
        self.event_type == event_type
    }

    /// 检查事件是否来自指定源
    /// Check if event is from specified source
    pub fn is_from(&self, source: &str) -> bool {
        self.source == source
    }

    /// 检查事件类型是否匹配前缀
    /// Check if event type matches prefix
    pub fn matches_prefix(&self, prefix: &str) -> bool {
        self.event_type.starts_with(prefix)
    }
}

// ============================================================================
// EventBuilder - 事件构建器
// EventBuilder - Event Builder
// ============================================================================

/// 事件构建器
/// Event Builder
///
/// 提供流式 API 来构建事件。
/// Provides a fluent API to build events.
pub struct EventBuilder {
    event: GlobalEvent,
}

impl EventBuilder {
    /// 创建新构建器
    /// Create a new builder
    pub fn new(event_type: impl Into<String>, source: impl Into<String>) -> Self {
        Self {
            event: GlobalEvent::new(event_type, source),
        }
    }

    /// 设置事件 ID
    /// Set event ID
    pub fn id(mut self, id: impl Into<String>) -> Self {
        self.event.event_id = id.into();
        self
    }

    /// 设置事件数据
    /// Set event data
    pub fn data(mut self, data: impl Into<serde_json::Value>) -> Self {
        self.event.data = data.into();
        self
    }

    /// 添加元数据
    /// Add metadata
    pub fn metadata(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.event.metadata.insert(key.into(), value.into());
        self
    }

    /// 设置时间戳
    /// Set timestamp
    pub fn timestamp(mut self, timestamp: u64) -> Self {
        self.event.timestamp = timestamp;
        self
    }

    /// 构建事件
    /// Build event
    #[must_use]
    pub fn build(self) -> GlobalEvent {
        self.event
    }
}

// ============================================================================
// 预定义事件类型常量
// Predefined Event Type Constants
// ============================================================================

/// 生命周期事件类型
/// Lifecycle event types
pub mod lifecycle {
    pub const CREATED: &str = "lifecycle:created";
    pub const INITIALIZED: &str = "lifecycle:initialized";
    pub const STARTED: &str = "lifecycle:started";
    pub const STOPPED: &str = "lifecycle:stopped";
    pub const SHUTDOWN: &str = "lifecycle:shutdown";
    pub const DESTROYED: &str = "lifecycle:destroyed";
}

/// 执行事件类型
/// Execution event types
pub mod execution {
    pub const STARTED: &str = "execution:started";
    pub const COMPLETED: &str = "execution:completed";
    pub const FAILED: &str = "execution:failed";
    pub const INTERRUPTED: &str = "execution:interrupted";
    pub const TIMEOUT: &str = "execution:timeout";
}

/// 消息事件类型
/// Message event types
pub mod message {
    pub const SENT: &str = "message:sent";
    pub const RECEIVED: &str = "message:received";
    pub const DELIVERED: &str = "message:delivered";
    pub const FAILED: &str = "message:failed";
}

/// 插件事件类型
/// Plugin event types
pub mod plugin {
    pub const LOADED: &str = "plugin:loaded";
    pub const UNLOADED: &str = "plugin:unloaded";
    pub const ERROR: &str = "plugin:error";
}

/// 状态事件类型
/// State event types
pub mod state {
    pub const CHANGED: &str = "state:changed";
    pub const ERROR: &str = "state:error";
    pub const PAUSED: &str = "state:paused";
    pub const RESUMED: &str = "state:resumed";
}

// ============================================================================
// 辅助函数：快速创建常见事件
// Helper Functions: Quick Creation of Common Events
// ============================================================================

/// 创建生命周期事件
/// Create lifecycle event
pub fn lifecycle_event(event_type: &str, source: &str) -> GlobalEvent {
    GlobalEvent::new(event_type, source)
}

/// 创建执行事件
/// Create execution event
pub fn execution_event(event_type: &str, source: &str, data: serde_json::Value) -> GlobalEvent {
    GlobalEvent::new(event_type, source).with_data(data)
}

/// 创建状态变更事件
/// Create state change event
pub fn state_changed_event(source: &str, old_state: &str, new_state: &str) -> GlobalEvent {
    GlobalEvent::new(state::CHANGED, source).with_data(serde_json::json!({
        "old_state": old_state,
        "new_state": new_state
    }))
}

/// 创建错误事件
/// Create error event
pub fn error_event(source: &str, error: &str) -> GlobalEvent {
    GlobalEvent::new("error", source).with_data(serde_json::json!({
        "error": error
    }))
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_event_creation() {
        let event = GlobalEvent::new("test:event", "agent1");
        assert_eq!(event.event_type, "test:event");
        assert_eq!(event.source, "agent1");
        assert!(!event.event_id.is_empty());
    }

    #[test]
    fn test_event_with_data() {
        let data = serde_json::json!({ "key": "value" });
        let event = GlobalEvent::new("test:event", "agent1").with_data(data.clone());

        assert_eq!(event.data, data);
    }

    #[test]
    fn test_event_builder() {
        let event = EventBuilder::new("test:event", "agent1")
            .id("custom-id")
            .data(serde_json::json!({ "test": true }))
            .metadata("meta1", "value1")
            .timestamp(12345)
            .build();

        assert_eq!(event.event_id, "custom-id");
        assert_eq!(event.timestamp, 12345);
        assert_eq!(event.metadata.get("meta1"), Some(&"value1".to_string()));
    }

    #[test]
    fn test_event_type_checks() {
        let event = GlobalEvent::new("lifecycle:initialized", "agent1");

        assert!(event.is_type("lifecycle:initialized"));
        assert!(event.is_from("agent1"));
        assert!(event.matches_prefix("lifecycle:"));
        assert!(!event.matches_prefix("execution:"));
    }

    #[test]
    fn test_helper_functions() {
        let event = state_changed_event("agent1", "ready", "executing");

        assert!(event.is_type(state::CHANGED));
        assert_eq!(event.source, "agent1");

        let data: serde_json::Value = event.data;
        assert_eq!(data["old_state"], "ready");
        assert_eq!(data["new_state"], "executing");
    }
}
