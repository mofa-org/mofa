//! 全局消息协议
//!
//! 本模块提供全局抽象消息协议，用于替代多个重复的 AgentMessage 定义。
//!
//! # 设计目标
//!
//! - 提供单一的消息类型，避免多处重复定义
//! - 支持多种通信模式（点对点、广播、请求-响应、发布-订阅）
//! - 支持多种内容格式（文本、JSON、二进制、结构化数据）
//! - 类型安全且可序列化

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ============================================================================
// GlobalMessage - 全局消息类型
// ============================================================================

/// 全局消息类型
///
/// 替代多处重复的 `AgentMessage` 定义，提供全局消息抽象。
///
/// # 消息模式
///
/// - `Direct`: 点对点直接消息
/// - `Broadcast`: 广播消息到所有订阅者
/// - `Request`: 请求消息（期待响应）
/// - `Response`: 响应消息
/// - `PubSub`: 发布-订阅模式
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum GlobalMessage {
    /// 点对点直接消息
    Direct {
        /// 发送者 ID
        sender: String,
        /// 接收者 ID
        recipient: String,
        /// 消息内容
        content: MessageContent,
    },

    /// 广播消息
    Broadcast {
        /// 发送者 ID
        sender: String,
        /// 主题
        topic: String,
        /// 消息内容
        content: MessageContent,
    },

    /// 请求消息（期待响应）
    Request {
        /// 发送者 ID
        sender: String,
        /// 接收者 ID
        recipient: String,
        /// 请求 ID（用于匹配响应）
        request_id: String,
        /// 消息内容
        content: MessageContent,
        /// 是否期待响应
        expect_reply: bool,
    },

    /// 响应消息
    Response {
        /// 响应者 ID
        responder: String,
        /// 请求 ID（用于匹配原始请求）
        request_id: String,
        /// 消息内容
        content: MessageContent,
    },

    /// 发布-订阅消息
    PubSub {
        /// 发布者 ID
        publisher: String,
        /// 主题
        topic: String,
        /// 消息内容
        content: MessageContent,
    },
}

impl GlobalMessage {
    /// 获取消息发送者 ID
    pub fn sender(&self) -> &str {
        match self {
            Self::Direct { sender, .. }
            | Self::Broadcast { sender, .. }
            | Self::Request { sender, .. } => sender,
            Self::Response { responder, .. } => responder,
            Self::PubSub { publisher, .. } => publisher,
        }
    }

    /// 获取消息类型标识
    pub fn message_type(&self) -> &'static str {
        match self {
            Self::Direct { .. } => "direct",
            Self::Broadcast { .. } => "broadcast",
            Self::Request { .. } => "request",
            Self::Response { .. } => "response",
            Self::PubSub { .. } => "pubsub",
        }
    }

    /// 创建点对点消息
    pub fn direct(sender: impl Into<String>, recipient: impl Into<String>, content: MessageContent) -> Self {
        Self::Direct {
            sender: sender.into(),
            recipient: recipient.into(),
            content,
        }
    }

    /// 创建广播消息
    pub fn broadcast(sender: impl Into<String>, topic: impl Into<String>, content: MessageContent) -> Self {
        Self::Broadcast {
            sender: sender.into(),
            topic: topic.into(),
            content,
        }
    }

    /// 创建请求消息
    pub fn request(
        sender: impl Into<String>,
        recipient: impl Into<String>,
        request_id: impl Into<String>,
        content: MessageContent,
    ) -> Self {
        Self::Request {
            sender: sender.into(),
            recipient: recipient.into(),
            request_id: request_id.into(),
            content,
            expect_reply: true,
        }
    }

    /// 创建响应消息
    pub fn response(
        responder: impl Into<String>,
        request_id: impl Into<String>,
        content: MessageContent,
    ) -> Self {
        Self::Response {
            responder: responder.into(),
            request_id: request_id.into(),
            content,
        }
    }
}

// ============================================================================
// MessageContent - 消息内容
// ============================================================================

/// 消息内容类型
///
/// 支持多种内容格式，提供灵活的消息传递能力。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MessageContent {
    /// 纯文本内容
    Text(String),

    /// JSON 数据
    Json(serde_json::Value),

    /// 二进制数据
    Binary(Vec<u8>),

    /// 结构化数据（带类型标识）
    Structured {
        /// 消息类型标识
        msg_type: String,
        /// 数据
        data: serde_json::Value,
    },
}

impl MessageContent {
    /// 创建文本内容
    pub fn text(text: impl Into<String>) -> Self {
        Self::Text(text.into())
    }

    /// 创建 JSON 内容
    pub fn json(value: serde_json::Value) -> Self {
        Self::Json(value)
    }

    /// 创建二进制内容
    pub fn binary(data: Vec<u8>) -> Self {
        Self::Binary(data)
    }

    /// 创建结构化内容
    pub fn structured(msg_type: impl Into<String>, data: serde_json::Value) -> Self {
        Self::Structured {
            msg_type: msg_type.into(),
            data,
        }
    }

    /// 转换为文本表示
    pub fn to_text(&self) -> String {
        match self {
            Self::Text(s) => s.clone(),
            Self::Json(v) => v.to_string(),
            Self::Binary(b) => format!("[binary {} bytes]", b.len()),
            Self::Structured { msg_type, data } => format!("{}: {}", msg_type, data),
        }
    }

    /// 尝试获取文本内容
    pub fn as_text(&self) -> Option<&str> {
        match self {
            Self::Text(s) => Some(s),
            _ => None,
        }
    }

    /// 尝试获取 JSON 内容
    pub fn as_json(&self) -> Option<&serde_json::Value> {
        match self {
            Self::Json(v) => Some(v),
            Self::Structured { data, .. } => Some(data),
            _ => None,
        }
    }

    /// 尝试获取二进制内容
    pub fn as_binary(&self) -> Option<&[u8]> {
        match self {
            Self::Binary(b) => Some(b),
            _ => None,
        }
    }
}

impl From<String> for MessageContent {
    fn from(s: String) -> Self {
        Self::Text(s)
    }
}

impl From<&str> for MessageContent {
    fn from(s: &str) -> Self {
        Self::Text(s.to_string())
    }
}

impl From<serde_json::Value> for MessageContent {
    fn from(v: serde_json::Value) -> Self {
        Self::Json(v)
    }
}

impl From<Vec<u8>> for MessageContent {
    fn from(v: Vec<u8>) -> Self {
        Self::Binary(v)
    }
}

// ============================================================================
// MessageMetadata - 消息元数据
// ============================================================================

/// 消息元数据
///
/// 用于携带额外的消息属性。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageMetadata {
    /// 消息 ID
    pub id: String,

    /// 时间戳（毫秒）
    pub timestamp: u64,

    /// 自定义属性
    pub properties: HashMap<String, String>,
}

impl Default for MessageMetadata {
    fn default() -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_millis() as u64,
            properties: HashMap::new(),
        }
    }
}

impl MessageMetadata {
    /// 创建新的元数据
    pub fn new() -> Self {
        Self::default()
    }

    /// 添加属性
    pub fn with_property(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.properties.insert(key.into(), value.into());
        self
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_message_content_text() {
        let content = MessageContent::text("Hello, World!");
        assert_eq!(content.as_text(), Some("Hello, World!"));
        assert_eq!(content.to_text(), "Hello, World!");
    }

    #[test]
    fn test_message_content_json() {
        let json = serde_json::json!({ "key": "value" });
        let content = MessageContent::json(json.clone());
        assert_eq!(content.as_json(), Some(&json));
    }

    #[test]
    fn test_global_message_direct() {
        let msg = GlobalMessage::direct("agent1", "agent2", MessageContent::text("test"));
        assert_eq!(msg.sender(), "agent1");
        assert_eq!(msg.message_type(), "direct");
    }

    #[test]
    fn test_global_message_request_response() {
        let request = GlobalMessage::request(
            "client",
            "server",
            "req-123",
            MessageContent::text("ping"),
        );

        let response = GlobalMessage::response(
            "server",
            "req-123",
            MessageContent::text("pong"),
        );

        assert_eq!(request.message_type(), "request");
        assert_eq!(response.message_type(), "response");
    }

    #[test]
    fn test_message_from_conversions() {
        let _: MessageContent = "hello".into();
        let _: MessageContent = String::from("world").into();
        let _: MessageContent = serde_json::json!(42).into();
        let _: MessageContent = vec![1, 2, 3].into();
    }
}
