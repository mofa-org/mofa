//! 全局消息协议
//! Global Message Protocol
//!
//! 本模块提供全局抽象消息协议，用于替代多个重复的 AgentMessage 定义。
//! This module provides a global abstract message protocol to replace redundant AgentMessage definitions.
//!
//! # 设计目标
//! # Design Goals
//!
//! - 提供单一的消息类型，避免多处重复定义
//! - Provide a single message type to avoid duplicate definitions across the codebase
//! - 支持多种通信模式（点对点、广播、请求-响应、发布-订阅）
//! - Support various communication patterns (P2P, Broadcast, Req-Res, Pub-Sub)
//! - 支持多种内容格式（文本、JSON、二进制、结构化数据）
//! - Support multiple content formats (Text, JSON, Binary, Structured Data)
//! - 类型安全且可序列化
//! - Type-safe and serializable

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ============================================================================
// GlobalMessage - 全局消息类型
// GlobalMessage - Global Message Type
// ============================================================================

/// 全局消息类型
/// Global message type
///
/// 替代多处重复的 `AgentMessage` 定义，提供全局消息抽象。
/// Replaces redundant `AgentMessage` definitions, providing a global message abstraction.
///
/// # 消息模式
/// # Message Patterns
///
/// - `Direct`: 点对点直接消息
/// - `Direct`: Point-to-point direct message
/// - `Broadcast`: 广播消息到所有订阅者
/// - `Broadcast`: Broadcast message to all subscribers
/// - `Request`: 请求消息（期待响应）
/// - `Request`: Request message (expects a response)
/// - `Response`: 响应消息
/// - `Response`: Response message
/// - `PubSub`: 发布-订阅模式
/// - `PubSub`: Publish-Subscribe pattern
#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub enum GlobalMessage {
    /// 点对点直接消息
    /// Point-to-point direct message
    Direct {
        /// 发送者 ID
        /// Sender ID
        sender: String,
        /// 接收者 ID
        /// Recipient ID
        recipient: String,
        /// 消息内容
        /// Message content
        content: MessageContent,
    },

    /// 广播消息
    /// Broadcast message
    Broadcast {
        /// 发送者 ID
        /// Sender ID
        sender: String,
        /// 主题
        /// Topic
        topic: String,
        /// 消息内容
        /// Message content
        content: MessageContent,
    },

    /// 请求消息（期待响应）
    /// Request message (expects response)
    Request {
        /// 发送者 ID
        /// Sender ID
        sender: String,
        /// 接收者 ID
        /// Recipient ID
        recipient: String,
        /// 请求 ID（用于匹配响应）
        /// Request ID (for matching responses)
        request_id: String,
        /// 消息内容
        /// Message content
        content: MessageContent,
        /// 是否期待响应
        /// Whether a reply is expected
        expect_reply: bool,
    },

    /// 响应消息
    /// Response message
    Response {
        /// 响应者 ID
        /// Responder ID
        responder: String,
        /// 请求 ID（用于匹配原始请求）
        /// Request ID (for matching original request)
        request_id: String,
        /// 消息内容
        /// Message content
        content: MessageContent,
    },

    /// 发布-订阅消息
    /// Publish-Subscribe message
    PubSub {
        /// 发布者 ID
        /// Publisher ID
        publisher: String,
        /// 主题
        /// Topic
        topic: String,
        /// 消息内容
        /// Message content
        content: MessageContent,
    },
}

impl GlobalMessage {
    /// 获取消息发送者 ID
    /// Get message sender ID
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
    /// Get message type identifier
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
    /// Create direct message
    pub fn direct(
        sender: impl Into<String>,
        recipient: impl Into<String>,
        content: MessageContent,
    ) -> Self {
        Self::Direct {
            sender: sender.into(),
            recipient: recipient.into(),
            content,
        }
    }

    /// 创建广播消息
    /// Create broadcast message
    pub fn broadcast(
        sender: impl Into<String>,
        topic: impl Into<String>,
        content: MessageContent,
    ) -> Self {
        Self::Broadcast {
            sender: sender.into(),
            topic: topic.into(),
            content,
        }
    }

    /// 创建请求消息
    /// Create request message
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
    /// Create response message
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
// MessageContent - Message Content
// ============================================================================

/// 消息内容类型
/// Message content type
///
/// 支持多种内容格式，提供灵活的消息传递能力。
/// Supports multiple content formats, providing flexible message delivery capabilities.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub enum MessageContent {
    /// 纯文本内容
    /// Plain text content
    Text(String),

    /// JSON 数据
    /// JSON data
    Json(serde_json::Value),

    /// 二进制数据
    /// Binary data
    Binary(Vec<u8>),

    /// 结构化数据（带类型标识）
    /// Structured data (with type identifier)
    Structured {
        /// 消息类型标识
        /// Message type identifier
        msg_type: String,
        /// 数据
        /// Data
        data: serde_json::Value,
    },
}

impl MessageContent {
    /// 创建文本内容
    /// Create text content
    pub fn text(text: impl Into<String>) -> Self {
        Self::Text(text.into())
    }

    /// 创建 JSON 内容
    /// Create JSON content
    pub fn json(value: serde_json::Value) -> Self {
        Self::Json(value)
    }

    /// 创建二进制内容
    /// Create binary content
    pub fn binary(data: Vec<u8>) -> Self {
        Self::Binary(data)
    }

    /// 创建结构化内容
    /// Create structured content
    pub fn structured(msg_type: impl Into<String>, data: serde_json::Value) -> Self {
        Self::Structured {
            msg_type: msg_type.into(),
            data,
        }
    }

    /// 转换为文本表示
    /// Convert to text representation
    pub fn to_text(&self) -> String {
        match self {
            Self::Text(s) => s.clone(),
            Self::Json(v) => v.to_string(),
            Self::Binary(b) => format!("[binary {} bytes]", b.len()),
            Self::Structured { msg_type, data } => format!("{}: {}", msg_type, data),
        }
    }

    /// 尝试获取文本内容
    /// Try to get text content
    pub fn as_text(&self) -> Option<&str> {
        match self {
            Self::Text(s) => Some(s),
            _ => None,
        }
    }

    /// 尝试获取 JSON 内容
    /// Try to get JSON content
    pub fn as_json(&self) -> Option<&serde_json::Value> {
        match self {
            Self::Json(v) => Some(v),
            Self::Structured { data, .. } => Some(data),
            _ => None,
        }
    }

    /// 尝试获取二进制内容
    /// Try to get binary content
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
// MessageMetadata - Message Metadata
// ============================================================================

/// 消息元数据
/// Message metadata
///
/// 用于携带额外的消息属性。
/// Used to carry additional message properties.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageMetadata {
    /// 消息 ID
    /// Message ID
    pub id: String,

    /// 时间戳（毫秒）
    /// Timestamp (milliseconds)
    pub timestamp: u64,

    /// 自定义属性
    /// Custom properties
    pub properties: HashMap<String, String>,
}

impl Default for MessageMetadata {
    fn default() -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            timestamp: crate::utils::now_ms(),
            properties: HashMap::new(),
        }
    }
}

impl MessageMetadata {
    /// 创建新的元数据
    /// Create new metadata
    pub fn new() -> Self {
        Self::default()
    }

    /// 添加属性
    /// Add property
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
        /// 验证文本消息内容
        /// Validate text message content
        let content = MessageContent::text("Hello, World!");
        assert_eq!(content.as_text(), Some("Hello, World!"));
        assert_eq!(content.to_text(), "Hello, World!");
    }

    #[test]
    fn test_message_content_json() {
        /// 验证 JSON 消息内容
        /// Validate JSON message content
        let json = serde_json::json!({ "key": "value" });
        let content = MessageContent::json(json.clone());
        assert_eq!(content.as_json(), Some(&json));
    }

    #[test]
    fn test_global_message_direct() {
        /// 验证点对点全局消息
        /// Validate P2P global message
        let msg = GlobalMessage::direct("agent1", "agent2", MessageContent::text("test"));
        assert_eq!(msg.sender(), "agent1");
        assert_eq!(msg.message_type(), "direct");
    }

    #[test]
    fn test_global_message_request_response() {
        /// 验证请求-响应流程
        /// Validate Request-Response flow
        let request =
            GlobalMessage::request("client", "server", "req-123", MessageContent::text("ping"));

        let response = GlobalMessage::response("server", "req-123", MessageContent::text("pong"));

        assert_eq!(request.message_type(), "request");
        assert_eq!(response.message_type(), "response");
    }

    #[test]
    fn test_message_from_conversions() {
        /// 验证内容类型转换
        /// Validate content type conversions
        let _: MessageContent = "hello".into();
        let _: MessageContent = String::from("world").into();
        let _: MessageContent = serde_json::json!(42).into();
        let _: MessageContent = vec![1, 2, 3].into();
    }
}
