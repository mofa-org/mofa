//! 全局事件系统
//!
//! 本模块提供全局事件系统，用于替代分散的 AgentEvent 和 PluginEvent 定义。
//!
//! # 设计目标
//!
//! - 提供单一的事件类型，避免多处重复定义
//! - 支持事件的发布-订阅模式
//! - 支持事件携带任意数据
//! - 提供清晰的事件来源和类型标识

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ============================================================================
// GlobalEvent - 全局事件类型
// ============================================================================

/// 全局事件类型
///
/// 替代 `AgentEvent` 和 `PluginEvent`，提供全局事件抽象。
///
/// # 事件类型
///
/// 常见的事件类型包括：
/// - `lifecycle:*` - 生命周期事件（created, initialized, started, stopped, shutdown）
/// - `execution:*` - 执行事件（started, completed, failed, interrupted）
/// - `message:*` - 消息事件（sent, received, delivered）
/// - `plugin:*` - 插件事件（loaded, unloaded, error）
/// - `state:*` - 状态变更事件（changed, error）
/// - `custom:*` - 自定义事件
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GlobalEvent {
    /// 事件 ID（唯一标识）
    pub event_id: String,

    /// 事件类型
    ///
    /// 使用命名空间格式，如 "lifecycle:initialized"
    pub event_type: String,

    /// 事件源（触发事件的 Agent 或组件 ID）
    pub source: String,

    /// 时间戳（毫秒）
    pub timestamp: u64,

    /// 事件数据（负载）
    pub data: serde_json::Value,

    /// 元数据（额外属性）
    pub metadata: HashMap<String, String>,
}

impl GlobalEvent {
    /// 创建新事件
    pub fn new(event_type: impl Into<String>, source: impl Into<String>) -> Self {
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;

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
    pub fn with_data(mut self, data: impl Into<serde_json::Value>) -> Self {
        self.data = data.into();
        self
    }

    /// 添加元数据
    pub fn with_metadata(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.metadata.insert(key.into(), value.into());
        self
    }

    /// 获取特定数据字段
    pub fn get_data<T: for<'de> Deserialize<'de>>(&self) -> Option<T> {
        serde_json::from_value(self.data.clone()).ok()
    }

    /// 检查事件是否为指定类型
    pub fn is_type(&self, event_type: &str) -> bool {
        self.event_type == event_type
    }

    /// 检查事件是否来自指定源
    pub fn is_from(&self, source: &str) -> bool {
        self.source == source
    }

    /// 检查事件类型是否匹配前缀
    pub fn matches_prefix(&self, prefix: &str) -> bool {
        self.event_type.starts_with(prefix)
    }
}

// ============================================================================
// EventBuilder - 事件构建器
// ============================================================================

/// 事件构建器
///
/// 提供流式 API 来构建事件。
pub struct EventBuilder {
    event: GlobalEvent,
}

impl EventBuilder {
    /// 创建新构建器
    pub fn new(event_type: impl Into<String>, source: impl Into<String>) -> Self {
        Self {
            event: GlobalEvent::new(event_type, source),
        }
    }

    /// 设置事件 ID
    pub fn id(mut self, id: impl Into<String>) -> Self {
        self.event.event_id = id.into();
        self
    }

    /// 设置事件数据
    pub fn data(mut self, data: impl Into<serde_json::Value>) -> Self {
        self.event.data = data.into();
        self
    }

    /// 添加元数据
    pub fn metadata(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.event.metadata.insert(key.into(), value.into());
        self
    }

    /// 设置时间戳
    pub fn timestamp(mut self, timestamp: u64) -> Self {
        self.event.timestamp = timestamp;
        self
    }

    /// 构建事件
    pub fn build(self) -> GlobalEvent {
        self.event
    }
}

// ============================================================================
// 预定义事件类型常量
// ============================================================================

/// 生命周期事件类型
pub mod lifecycle {
    pub const CREATED: &str = "lifecycle:created";
    pub const INITIALIZED: &str = "lifecycle:initialized";
    pub const STARTED: &str = "lifecycle:started";
    pub const STOPPED: &str = "lifecycle:stopped";
    pub const SHUTDOWN: &str = "lifecycle:shutdown";
    pub const DESTROYED: &str = "lifecycle:destroyed";
}

/// 执行事件类型
pub mod execution {
    pub const STARTED: &str = "execution:started";
    pub const COMPLETED: &str = "execution:completed";
    pub const FAILED: &str = "execution:failed";
    pub const INTERRUPTED: &str = "execution:interrupted";
    pub const TIMEOUT: &str = "execution:timeout";
}

/// 消息事件类型
pub mod message {
    pub const SENT: &str = "message:sent";
    pub const RECEIVED: &str = "message:received";
    pub const DELIVERED: &str = "message:delivered";
    pub const FAILED: &str = "message:failed";
}

/// 插件事件类型
pub mod plugin {
    pub const LOADED: &str = "plugin:loaded";
    pub const UNLOADED: &str = "plugin:unloaded";
    pub const ERROR: &str = "plugin:error";
}

/// 状态事件类型
pub mod state {
    pub const CHANGED: &str = "state:changed";
    pub const ERROR: &str = "state:error";
    pub const PAUSED: &str = "state:paused";
    pub const RESUMED: &str = "state:resumed";
}

// ============================================================================
// 辅助函数：快速创建常见事件
// ============================================================================

/// 创建生命周期事件
pub fn lifecycle_event(event_type: &str, source: &str) -> GlobalEvent {
    GlobalEvent::new(event_type, source)
}

/// 创建执行事件
pub fn execution_event(event_type: &str, source: &str, data: serde_json::Value) -> GlobalEvent {
    GlobalEvent::new(event_type, source).with_data(data)
}

/// 创建状态变更事件
pub fn state_changed_event(source: &str, old_state: &str, new_state: &str) -> GlobalEvent {
    GlobalEvent::new(state::CHANGED, source).with_data(serde_json::json!({
        "old_state": old_state,
        "new_state": new_state
    }))
}

/// 创建错误事件
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
