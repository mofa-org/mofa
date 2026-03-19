//! Unified SSE response builder for OpenAI-compatible streaming.
//!
//! [`SseBuilder`] converts any [`BoxTokenStream`] into a properly formatted
//! Server-Sent Events HTTP response that is compatible with OpenAI API clients.
//!
//! # StreamChunk → SSE event mapping
//!
//! ```text
//!  BoxTokenStream item             SSE wire event
//! ─────────────────────────────── ──────────────────────────────────────────────────
//!  (builder opens)                data: {"choices":[{"delta":{"role":"assistant"}}]}
//!
//!  Ok(StreamChunk { delta: "Hi"   data: {"choices":[{"delta":{"content":"Hi"}}]}
//!                 , finish_reason: None })
//!
//!  Ok(StreamChunk { delta: ""     (skipped — empty delta, not a done marker)
//!                 , finish_reason: None })
//!
//!  Ok(StreamChunk { delta: ""     data: {"choices":[{"delta":{},"finish_reason":"stop"}]}
//!                 , finish_reason: Some(Stop) })
//!
//!  Err(StreamError::Provider{…})  data: {"error":{"message":"…","type":"stream_error"}}
//!
//!  (stream exhausted)             data: [DONE]
//! ─────────────────────────────── ──────────────────────────────────────────────────
//! ```
//!
//! # Keep-alive
//!
//! A periodic SSE `: keep-alive` comment is sent automatically so that
//! proxies and browsers do not close the connection during long inference runs.
//!
//! # Usage
//!
//! ```rust,no_run
//! use mofa_gateway::streaming::SseBuilder;
//! use mofa_kernel::llm::streaming::BoxTokenStream;
//! use axum::http::HeaderMap;
//!
//! fn handler(stream: BoxTokenStream, headers: HeaderMap) -> axum::response::Response {
//!     SseBuilder::new("gpt-4o")
//!         .with_headers(headers)   // forwards X-MoFA-* observability headers
//!         .build_response(stream)
//! }
//! ```

use std::convert::Infallible;

use axum::http::HeaderMap;
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::response::{IntoResponse, Response};
use futures::{StreamExt, stream};
use mofa_kernel::llm::streaming::{BoxTokenStream, StreamChunk, StreamError};
use tracing::error;

use crate::openai_compat::types::{ChatCompletionChunk, ChunkChoice, Delta};

// ─────────────────────────────────────────────────────────────────────────────
// Helpers
// ─────────────────────────────────────────────────────────────────────────────

/// Generate a pseudo-unique completion ID from the current timestamp.
fn completion_id() -> String {
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    format!("chatcmpl-mofa{ts}")
}

/// Current Unix timestamp in seconds.
fn unix_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Convert a [`mofa_kernel::llm::types::FinishReason`] to its OpenAI string form.
fn finish_reason_str(fr: &mofa_kernel::llm::types::FinishReason) -> &'static str {
    use mofa_kernel::llm::types::FinishReason;
    match fr {
        FinishReason::Stop => "stop",
        FinishReason::Length => "length",
        FinishReason::ToolCalls => "tool_calls",
        FinishReason::ContentFilter => "content_filter",
        _ => "stop",
    }
}

/// Serialize a [`ChatCompletionChunk`] to a JSON string for an SSE `data:` field.
/// Falls back to a safe error payload on serialization failure.
fn chunk_to_event(chunk: &ChatCompletionChunk) -> Event {
    match serde_json::to_string(chunk) {
        Ok(json) => Event::default().data(json),
        Err(e) => {
            error!(error = %e, "Failed to serialize SSE chunk");
            Event::default().data(
                r#"{"error":{"message":"internal serialization error","type":"server_error"}}"#,
            )
        }
    }
}

/// Build an SSE error event from a [`StreamError`].
fn error_to_event(err: &StreamError) -> Event {
    // Escape any double quotes in the message to keep JSON valid.
    let msg = err.to_string().replace('"', "\\\"");
    let payload = format!(r#"{{"error":{{"message":"{msg}","type":"stream_error"}}}}"#);
    Event::default().data(payload)
}

// ─────────────────────────────────────────────────────────────────────────────
// SseBuilder
// ─────────────────────────────────────────────────────────────────────────────

/// Unified SSE response builder for OpenAI-compatible streaming.
///
/// # Example
///
/// ```rust,no_run
/// use mofa_gateway::streaming::SseBuilder;
/// use mofa_kernel::llm::streaming::BoxTokenStream;
///
/// fn streaming_handler(stream: BoxTokenStream) -> axum::response::Response {
///     SseBuilder::new("gpt-4o")
///         .build_response(stream)
/// }
/// ```
#[derive(Debug, Clone)]
pub struct SseBuilder {
    /// Model name included in every chunk's `model` field.
    model: String,
    /// Unique completion ID shared across all chunks.
    id: String,
    /// Unix timestamp shared across all chunks.
    created: u64,
    /// Extra headers to attach to the response (e.g., `X-MoFA-Backend`).
    extra_headers: HeaderMap,
}

impl SseBuilder {
    /// Create a new builder for the given model name.
    pub fn new(model: impl Into<String>) -> Self {
        Self {
            model: model.into(),
            id: completion_id(),
            created: unix_now(),
            extra_headers: HeaderMap::new(),
        }
    }

    /// Attach additional response headers (e.g., observability headers).
    pub fn with_headers(mut self, headers: HeaderMap) -> Self {
        self.extra_headers = headers;
        self
    }

    /// Build a streaming SSE [`Response`] from a [`BoxTokenStream`].
    ///
    /// Emits:
    /// 1. A role chunk (`"role":"assistant"`)
    /// 2. One content chunk per non-empty `StreamChunk.delta`
    /// 3. A stop chunk when `StreamChunk.finish_reason` is set
    /// 4. A terminal `[DONE]` event
    ///
    /// Stream errors are forwarded as OpenAI-style error events.
    pub fn build_response(self, stream: BoxTokenStream) -> Response {
        let id = self.id.clone();
        let model = self.model.clone();
        let created = self.created;

        // ── 1. Role chunk ──────────────────────────────────────────────────
        let role_chunk = ChatCompletionChunk {
            id: id.clone(),
            object: "chat.completion.chunk".to_string(),
            created,
            model: model.clone(),
            choices: vec![ChunkChoice {
                index: 0,
                delta: Delta {
                    role: Some("assistant".to_string()),
                    content: None,
                },
                finish_reason: None,
            }],
        };
        let pre_stream =
            stream::once(async move { Ok::<Event, Infallible>(chunk_to_event(&role_chunk)) });

        // ── 2. Content + stop chunks (one per StreamChunk) ─────────────────
        let id2 = id.clone();
        let model2 = model.clone();

        let content_stream = stream.flat_map(move |result| {
            let id = id2.clone();
            let model = model2.clone();
            let mut events: Vec<Result<Event, Infallible>> = Vec::new();

            match result {
                Ok(chunk) => {
                    // Emit content event if delta is non-empty
                    if !chunk.delta.is_empty() {
                        let ev = ChatCompletionChunk {
                            id: id.clone(),
                            object: "chat.completion.chunk".to_string(),
                            created,
                            model: model.clone(),
                            choices: vec![ChunkChoice {
                                index: 0,
                                delta: Delta {
                                    role: None,
                                    content: Some(chunk.delta.clone()),
                                },
                                finish_reason: None,
                            }],
                        };
                        events.push(Ok(chunk_to_event(&ev)));
                    }

                    // Emit stop event if stream is finished
                    if let Some(ref reason) = chunk.finish_reason {
                        let stop_ev = ChatCompletionChunk {
                            id: id.clone(),
                            object: "chat.completion.chunk".to_string(),
                            created,
                            model: model.clone(),
                            choices: vec![ChunkChoice {
                                index: 0,
                                delta: Delta::default(),
                                finish_reason: Some(finish_reason_str(reason).to_string()),
                            }],
                        };
                        events.push(Ok(chunk_to_event(&stop_ev)));
                    }
                }
                Err(ref err) => {
                    error!(error = %err, "LLM stream error during SSE");
                    events.push(Ok(error_to_event(err)));
                }
            }

            stream::iter(events)
        });

        // ── 3. [DONE] terminator ──────────────────────────────────────────
        let done_stream =
            stream::once(async { Ok::<Event, Infallible>(Event::default().data("[DONE]")) });

        let full_stream = pre_stream.chain(content_stream).chain(done_stream);

        let mut resp = Sse::new(full_stream)
            .keep_alive(KeepAlive::default())
            .into_response();

        resp.headers_mut().extend(self.extra_headers);
        resp
    }

    /// Compatibility shim: build SSE from `Stream<Item = String>`.
    ///
    /// This supports the current `InferenceOrchestrator::infer_stream()` which
    /// still returns a simple word stream. Each string token is wrapped into a
    /// [`StreamChunk`] before being handed to [`build_response`].
    ///
    /// **This method will be removed once the orchestrator returns a proper
    /// [`BoxTokenStream`] from real LLM providers.**
    pub fn build_response_from_text_stream(
        self,
        text_stream: std::pin::Pin<Box<dyn futures::Stream<Item = String> + Send + Sync>>,
    ) -> Response {
        // Wrap each String token in a StreamChunk
        let token_stream: BoxTokenStream = Box::pin(
            text_stream.map(|word| Ok::<StreamChunk, StreamError>(StreamChunk::text(word))),
        );
        self.build_response(token_stream)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::to_bytes;
    use futures::stream;
    use mofa_kernel::llm::streaming::{StreamChunk, StreamError};
    use mofa_kernel::llm::types::FinishReason;

    fn make_stream(chunks: Vec<Result<StreamChunk, StreamError>>) -> BoxTokenStream {
        Box::pin(stream::iter(chunks))
    }

    fn stream_from_words(words: &[&str]) -> BoxTokenStream {
        let items: Vec<_> = words
            .iter()
            .map(|w| Ok::<StreamChunk, StreamError>(StreamChunk::text(*w)))
            .collect();
        make_stream(items)
    }

    /// Collect all SSE `data:` lines from a response body.
    async fn collect_sse_data(resp: Response) -> Vec<String> {
        let bytes = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let text = String::from_utf8(bytes.to_vec()).unwrap();
        text.lines()
            .filter_map(|line| line.strip_prefix("data: ").map(str::to_string))
            .collect()
    }

    #[tokio::test]
    async fn test_sse_starts_with_role_chunk_and_ends_with_done() {
        let s = stream_from_words(&["Hello", " world"]);
        let resp = SseBuilder::new("test-model").build_response(s);

        let data = collect_sse_data(resp).await;
        assert!(!data.is_empty(), "Expected SSE events");

        // First event should contain the assistant role
        assert!(
            data[0].contains(r#""role":"assistant""#),
            "Expected role chunk first"
        );
        // Last event should be [DONE]
        assert_eq!(data.last().unwrap(), "[DONE]");
    }

    #[tokio::test]
    async fn test_sse_content_chunks_present() {
        let s = stream_from_words(&["Hi", " there"]);
        let resp = SseBuilder::new("test-model").build_response(s);

        let data = collect_sse_data(resp).await;
        let content_events: Vec<_> = data.iter().filter(|d| d.contains(r#""content""#)).collect();
        assert_eq!(content_events.len(), 2, "Expected two content chunks");
        assert!(content_events[0].contains("Hi"));
        assert!(content_events[1].contains(" there"));
    }

    #[tokio::test]
    async fn test_sse_finish_reason_emitted() {
        let chunks = vec![
            Ok(StreamChunk::text("Hello")),
            Ok(StreamChunk::done(FinishReason::Stop)),
        ];
        let s = make_stream(chunks);
        let resp = SseBuilder::new("test-model").build_response(s);

        let data = collect_sse_data(resp).await;
        let stop_events: Vec<_> = data
            .iter()
            .filter(|d| d.contains(r#""finish_reason""#))
            .collect();
        assert!(!stop_events.is_empty(), "Expected a stop event");
        assert!(stop_events[0].contains(r#""stop""#));
    }

    #[tokio::test]
    async fn test_sse_error_propagation() {
        let chunks = vec![
            Ok(StreamChunk::text("partial")),
            Err(StreamError::provider("openai", "rate limited")),
        ];
        let s = make_stream(chunks);
        let resp = SseBuilder::new("test-model").build_response(s);

        let data = collect_sse_data(resp).await;
        let error_events: Vec<_> = data
            .iter()
            .filter(|d| d.contains(r#""stream_error""#))
            .collect();
        assert!(!error_events.is_empty(), "Expected an error event");
        // Stream should still terminate with [DONE]
        assert_eq!(data.last().unwrap(), "[DONE]");
    }

    #[tokio::test]
    async fn test_sse_empty_chunks_are_skipped() {
        let chunks = vec![
            Ok(StreamChunk::text("")), // empty delta, not done → should be skipped
            Ok(StreamChunk::text("hello")),
        ];
        let s = make_stream(chunks);
        let resp = SseBuilder::new("test-model").build_response(s);

        let data = collect_sse_data(resp).await;
        let content_events: Vec<_> = data.iter().filter(|d| d.contains(r#""content""#)).collect();
        // Only "hello" should appear, empty string should be skipped
        assert_eq!(content_events.len(), 1);
        assert!(content_events[0].contains("hello"));
    }

    #[tokio::test]
    async fn test_text_stream_shim_works() {
        let words = vec!["foo".to_string(), "bar".to_string()];
        let text_stream: std::pin::Pin<Box<dyn futures::Stream<Item = String> + Send + Sync>> =
            Box::pin(stream::iter(words));
        let resp = SseBuilder::new("test-model").build_response_from_text_stream(text_stream);

        let data = collect_sse_data(resp).await;
        assert_eq!(data.last().unwrap(), "[DONE]");
        let content: Vec<_> = data.iter().filter(|d| d.contains(r#""content""#)).collect();
        assert_eq!(content.len(), 2);
    }

    #[test]
    fn test_finish_reason_str_mapping() {
        use mofa_kernel::llm::types::FinishReason;
        assert_eq!(finish_reason_str(&FinishReason::Stop), "stop");
        assert_eq!(finish_reason_str(&FinishReason::Length), "length");
        assert_eq!(finish_reason_str(&FinishReason::ToolCalls), "tool_calls");
        assert_eq!(
            finish_reason_str(&FinishReason::ContentFilter),
            "content_filter"
        );
    }
}
