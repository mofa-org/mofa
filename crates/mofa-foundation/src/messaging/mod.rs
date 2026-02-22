//! Generic message bus framework for decoupled agent architectures
//!
//! This module provides:
//! - Generic message types with pub/sub patterns
//! - Inbound/outbound message separation
//! - Trait-based message contracts
//! - Broadcast channel implementation

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{RwLock, broadcast};

/// Generic message bus for bidirectional messaging
#[derive(Clone)]
pub struct MessageBus<T, U>
where
    T: Clone + Send + 'static,
    U: Clone + Send + 'static,
{
    /// Inbound message sender (e.g., from channels to agent)
    inbound: broadcast::Sender<T>,
    /// Outbound message sender (e.g., from agent to channels)
    outbound: broadcast::Sender<U>,
    /// Outbound subscribers keyed by routing key
    outbound_subscribers: Arc<RwLock<HashMap<String, Vec<OutboundCallback<U>>>>>,
}

type OutboundCallback<U> =
    Arc<dyn Fn(U) -> std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send>> + Send + Sync>;

impl<T, U> MessageBus<T, U>
where
    T: Clone + Send + 'static,
    U: Clone + Send + 'static,
{
    /// Create a new message bus with specified capacity
    pub fn new(capacity: usize) -> Self {
        let (inbound_tx, _) = broadcast::channel(capacity);
        let (outbound_tx, _) = broadcast::channel(capacity);

        Self {
            inbound: inbound_tx,
            outbound: outbound_tx,
            outbound_subscribers: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Create with default capacity (100)
    pub fn default_capacity() -> Self {
        Self::new(100)
    }

    /// Publish an inbound message
    pub async fn publish_inbound(&self, msg: T) -> Result<(), broadcast::error::SendError<T>> {
        self.inbound.send(msg)?;
        Ok(())
    }

    /// Subscribe to inbound messages
    pub fn subscribe_inbound(&self) -> broadcast::Receiver<T> {
        self.inbound.subscribe()
    }

    /// Publish an outbound message
    pub async fn publish_outbound(&self, msg: U) -> Result<(), broadcast::error::SendError<U>> {
        self.outbound.send(msg)?;
        Ok(())
    }

    /// Subscribe to outbound messages
    pub fn subscribe_outbound(&self) -> broadcast::Receiver<U> {
        self.outbound.subscribe()
    }

    /// Subscribe to outbound messages for a specific routing key with a callback
    pub async fn subscribe_outbound_key<F, Fut>(&self, key: String, callback: F)
    where
        F: Fn(U) -> Fut + Send + Sync + 'static,
        Fut: std::future::Future<Output = ()> + Send + 'static,
    {
        let mut subscribers = self.outbound_subscribers.write().await;
        subscribers
            .entry(key)
            .or_insert_with(Vec::new)
            .push(Arc::new(move |msg| Box::pin(callback(msg))));
    }

    /// Get the number of inbound subscribers
    pub fn inbound_subscriber_count(&self) -> usize {
        self.inbound.receiver_count()
    }

    /// Get the number of outbound subscribers
    pub fn outbound_subscriber_count(&self) -> usize {
        self.outbound.receiver_count()
    }
}

impl<T, U> Default for MessageBus<T, U>
where
    T: Clone + Send + 'static,
    U: Clone + Send + 'static,
{
    fn default() -> Self {
        Self::default_capacity()
    }
}

/// Trait for inbound messages
pub trait InboundMessage: Clone + Send {
    /// Get the session key for this message
    fn session_key(&self) -> String;

    /// Get the message content
    fn content(&self) -> &str;

    /// Get media attachments (if any)
    fn media(&self) -> &[String] {
        &[]
    }

    /// Get metadata
    fn metadata(&self) -> &HashMap<String, serde_json::Value> {
        use std::sync::OnceLock;
        static EMPTY: OnceLock<HashMap<String, serde_json::Value>> = OnceLock::new();
        EMPTY.get_or_init(HashMap::new)
    }
}

/// Trait for outbound messages
pub trait OutboundMessage: Clone + Send {
    /// Get the target channel
    fn channel(&self) -> &str;

    /// Get the target chat ID
    fn chat_id(&self) -> &str;

    /// Get the message content
    fn content(&self) -> &str;

    /// Build a routing key from this message
    fn routing_key(&self) -> String {
        format!("{}:{}", self.channel(), self.chat_id())
    }
}

/// Simple inbound message implementation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SimpleInboundMessage {
    /// Channel identifier
    pub channel: String,
    /// Sender identifier
    pub sender_id: String,
    /// Chat/session identifier
    pub chat_id: String,
    /// Message content
    pub content: String,
    /// Media attachments
    #[serde(default)]
    pub media: Vec<String>,
    /// Additional metadata
    #[serde(flatten)]
    pub metadata: HashMap<String, serde_json::Value>,
}

impl InboundMessage for SimpleInboundMessage {
    fn session_key(&self) -> String {
        format!("{}:{}", self.channel, self.chat_id)
    }

    fn content(&self) -> &str {
        &self.content
    }

    fn media(&self) -> &[String] {
        &self.media
    }

    fn metadata(&self) -> &HashMap<String, serde_json::Value> {
        &self.metadata
    }
}

impl SimpleInboundMessage {
    /// Create a new simple inbound message
    pub fn new(
        channel: impl Into<String>,
        sender_id: impl Into<String>,
        chat_id: impl Into<String>,
        content: impl Into<String>,
    ) -> Self {
        Self {
            channel: channel.into(),
            sender_id: sender_id.into(),
            chat_id: chat_id.into(),
            content: content.into(),
            media: Vec::new(),
            metadata: HashMap::new(),
        }
    }

    /// Add media to the message
    pub fn with_media(mut self, media: Vec<String>) -> Self {
        self.media = media;
        self
    }

    /// Add metadata
    pub fn with_metadata(mut self, key: impl Into<String>, value: serde_json::Value) -> Self {
        self.metadata.insert(key.into(), value);
        self
    }
}

/// Simple outbound message implementation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SimpleOutboundMessage {
    /// Channel identifier
    pub channel: String,
    /// Chat/session identifier
    pub chat_id: String,
    /// Message content
    pub content: String,
    /// Optional reply-to message ID
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reply_to: Option<String>,
}

impl OutboundMessage for SimpleOutboundMessage {
    fn channel(&self) -> &str {
        &self.channel
    }

    fn chat_id(&self) -> &str {
        &self.chat_id
    }

    fn content(&self) -> &str {
        &self.content
    }
}

impl SimpleOutboundMessage {
    /// Create a new simple outbound message
    pub fn new(
        channel: impl Into<String>,
        chat_id: impl Into<String>,
        content: impl Into<String>,
    ) -> Self {
        Self {
            channel: channel.into(),
            chat_id: chat_id.into(),
            content: content.into(),
            reply_to: None,
        }
    }

    /// Set reply-to message ID
    pub fn with_reply_to(mut self, reply_to: impl Into<String>) -> Self {
        self.reply_to = Some(reply_to.into());
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_message_bus_publish() {
        let bus = MessageBus::<SimpleInboundMessage, SimpleOutboundMessage>::new(10);

        let mut rx = bus.subscribe_inbound();
        let msg = SimpleInboundMessage::new("test", "user", "chat", "Hello");

        tokio::spawn(async move {
            bus.publish_inbound(msg).await.unwrap();
        });

        let received = rx.recv().await.unwrap();
        assert_eq!(received.content, "Hello");
    }

    #[tokio::test]
    async fn test_multiple_subscribers() {
        let bus = MessageBus::<String, String>::new(10);

        let mut rx1 = bus.subscribe_inbound();
        let mut rx2 = bus.subscribe_inbound();

        bus.publish_inbound("test".to_string()).await.unwrap();

        let received1 = rx1.recv().await.unwrap();
        let received2 = rx2.recv().await.unwrap();

        assert_eq!(received1, "test");
        assert_eq!(received2, "test");
    }

    #[tokio::test]
    async fn test_outbound_subscribe() {
        let bus = MessageBus::<String, SimpleOutboundMessage>::new(10);

        let mut rx = bus.subscribe_outbound();
        let msg = SimpleOutboundMessage::new("telegram", "123", "Response");

        bus.publish_outbound(msg).await.unwrap();

        let received = rx.recv().await.unwrap();
        assert_eq!(received.content, "Response");
    }

    #[test]
    fn test_simple_inbound_message() {
        let msg = SimpleInboundMessage::new("telegram", "user123", "chat456", "Hello");
        assert_eq!(msg.session_key(), "telegram:chat456");
        assert_eq!(msg.content(), "Hello");
    }

    #[test]
    fn test_simple_inbound_message_with_media() {
        let msg = SimpleInboundMessage::new("telegram", "user123", "chat456", "Hello")
            .with_media(vec!["image.jpg".to_string()]);
        assert_eq!(msg.media().len(), 1);
    }

    #[test]
    fn test_simple_outbound_message() {
        let msg = SimpleOutboundMessage::new("telegram", "123", "Response");
        assert_eq!(msg.channel(), "telegram");
        assert_eq!(msg.chat_id(), "123");
        assert_eq!(msg.content(), "Response");
        assert_eq!(msg.routing_key(), "telegram:123");
    }

    #[test]
    fn test_simple_outbound_message_with_reply() {
        let msg = SimpleOutboundMessage::new("telegram", "123", "Response").with_reply_to("msg456");
        assert_eq!(msg.reply_to, Some("msg456".to_string()));
    }
}
