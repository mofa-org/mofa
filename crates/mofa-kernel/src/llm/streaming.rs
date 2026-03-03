//! Unified streaming types for provider-agnostic LLM token delivery

use futures::Stream;
use std::pin::Pin;

use super::types::{FinishReason, ToolCallDelta};

/// Provider agnostic streaming chunk
#[derive(Debug, Clone, Default)]
pub struct StreamChunk {
    /// Incremental text content
    pub delta: String,
    /// Reason the model stopped generating, if any
    pub finish_reason: Option<FinishReason>,
    /// Incremental usage counters
    pub usage: Option<UsageDelta>,
    /// Incremental tool-call data
    pub tool_calls: Option<Vec<ToolCallDelta>>,
}

impl StreamChunk {
    /// Text only chunk
    pub fn text(delta: impl Into<String>) -> Self {
        Self { delta: delta.into(), finish_reason: None, usage: None, tool_calls: None }
    }

    pub fn done(finish_reason: FinishReason) -> Self {
        Self { delta: String::new(), finish_reason: Some(finish_reason), usage: None, tool_calls: None }
    }

    pub fn is_done(&self) -> bool { self.finish_reason.is_some() }
}

/// Incremental token-usage counters
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct UsageDelta {
    pub prompt_tokens: Option<u32>,
    pub completion_tokens: Option<u32>,
    pub total_tokens: Option<u32>,
}

/// Streaming errors
#[derive(Debug, Clone, thiserror::Error)]
#[non_exhaustive]
pub enum StreamError {
    #[error("Provider '{provider}' error: {message}")]
    Provider { provider: String, message: String },
    #[error("Connection error: {0}")]
    Connection(String),
    #[error("Parse error: {0}")]
    Parse(String),
    #[error("Stream timeout: {0}")]
    Timeout(String),
    #[error("Stream cancelled")]
    Cancelled,
}

impl StreamError {
    pub fn provider(provider: impl Into<String>, message: impl Into<String>) -> Self {
        Self::Provider { provider: provider.into(), message: message.into() }
    }
}

/// Blanket trait for `Stream<Item = Result<StreamChunk, StreamError>> + Send`
pub trait TokenStream: Stream<Item = Result<StreamChunk, StreamError>> + Send {}
impl<T> TokenStream for T where T: Stream<Item = Result<StreamChunk, StreamError>> + Send {}

/// Type erased token stream
pub type BoxTokenStream = Pin<Box<dyn TokenStream>>;

#[cfg(test)]
mod tests {
    use super::*;
    use futures::StreamExt;

    #[test]
    fn chunk_constructors_and_predicates() {
        let t = StreamChunk::text("hello");
        assert_eq!(t.delta, "hello");
        assert!(!t.is_done());

        let d = StreamChunk::done(FinishReason::Stop);
        assert!(d.is_done());
        assert_eq!(d.finish_reason, Some(FinishReason::Stop));
    }

    #[test]
    fn stream_error_display() {
        assert_eq!(StreamError::Connection("reset".into()).to_string(), "Connection error: reset");
        assert_eq!(StreamError::Cancelled.to_string(), "Stream cancelled");
        assert_eq!(StreamError::provider("x", "y").to_string(), "Provider 'x' error: y");
    }

    #[tokio::test]
    async fn box_token_stream_roundtrip() {
        let items = vec![
            Ok(StreamChunk::text("Hi")),
            Err(StreamError::Connection("lost".into())),
            Ok(StreamChunk::done(FinishReason::Stop)),
        ];
        let mut s: BoxTokenStream = Box::pin(futures::stream::iter(items));

        assert_eq!(s.next().await.unwrap().unwrap().delta, "Hi");
        assert!(s.next().await.unwrap().is_err());
        assert!(s.next().await.unwrap().unwrap().is_done());
        assert!(s.next().await.is_none());
    }
}
