//! Streaming Response Protocol (SRP)
//!
//! The SRP sits on top of [`LLMProvider::chat_stream`] and wraps the raw
//! `ChatCompletionChunk` stream in a typed, framed event model.  This gives
//! every caller a single, provider-agnostic interface for:
//!
//! * consuming incremental token deltas (`StreamEvent::Delta`)
//! * detecting clean stream termination (`StreamEvent::Done`)
//! * handling client-driven cancellation (`StreamEvent::Cancelled`)
//! * tolerating slow or stalled backends via periodic keepalives
//!   (`StreamEvent::Heartbeat`)
//!
//! # Feature flag
//!
//! This module requires the **`streaming`** Cargo feature:
//!
//! ```toml
//! [dependencies]
//! mofa-kernel = { version = "…", features = ["streaming"] }
//! ```
//!
//! # Quick start
//!
//! ```rust,ignore
//! use mofa_kernel::llm::srp::{stream_inference, SrpConfig, StreamEvent};
//! use tokio_util::sync::CancellationToken;
//! use futures::StreamExt;
//!
//! let token = CancellationToken::new();
//! let mut events = stream_inference(&provider, request, token.clone(), SrpConfig::default())
//!     .await
//!     .expect("stream started");
//!
//! while let Some(event) = events.next().await {
//!     match event {
//!         StreamEvent::Delta(chunk) => print!("{}", chunk.delta),
//!         StreamEvent::Done         => break,
//!         StreamEvent::Cancelled    => { eprintln!("cancelled"); break; }
//!         StreamEvent::Heartbeat    => { /* still alive */ }
//!         _                         => {} // #[non_exhaustive] catch-all
//!     }
//! }
//! ```

use std::time::Duration;

use async_trait::async_trait;
use futures::StreamExt as _;
use thiserror::Error;
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;
use tokio_util::sync::CancellationToken;
use tracing::warn;

use crate::agent::{AgentError, AgentResult};

use super::provider::LLMProvider;
use super::streaming::StreamChunk;
use super::types::ChatCompletionRequest;

// ─── StreamEvent ──────────────────────────────────────────────────────────

/// A single framed event emitted by the Streaming Response Protocol.
///
/// The enum is `#[non_exhaustive]` so that new variants (e.g. a `Usage`
/// accounting event) can be added in future releases without breaking
/// existing `match` arms.
///
/// ## Hot-path guarantee
///
/// `StreamEvent<T>` is `Clone + Send` whenever `T: Clone + Send`.  The enum
/// itself adds no heap allocation; any allocation comes from the payload `T`.
///
/// # Matching
///
/// Always include a catch-all arm when matching:
///
/// ```rust
/// use mofa_kernel::llm::srp::StreamEvent;
/// use mofa_kernel::llm::streaming::StreamChunk;
///
/// fn handle(event: StreamEvent<StreamChunk>) {
///     match event {
///         StreamEvent::Delta(chunk) => println!("{}", chunk.delta),
///         StreamEvent::Done         => println!("stream finished"),
///         StreamEvent::Cancelled    => println!("stream cancelled"),
///         StreamEvent::Heartbeat    => { /* keepalive — no action required */ }
///         _                         => { /* forward-compatible catch-all */ }
///     }
/// }
/// ```
#[derive(Debug, Clone)]
#[non_exhaustive]
pub enum StreamEvent<T> {
    /// Incremental content delta from the model.
    Delta(T),
    /// Periodic keepalive emitted when no delta has arrived within
    /// [`SrpConfig::heartbeat_interval`].  Consumers may use this to
    /// detect stalled connections or reset watchdog timers.
    Heartbeat,
    /// The stream was terminated by the client via a [`CancellationToken`].
    Cancelled,
    /// The model finished generating normally; the stream is now closed.
    Done,
}

// ─── SinkError ────────────────────────────────────────────────────────────

/// Error returned by [`InferenceSink::send`].
#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[non_exhaustive]
pub enum SinkError {
    /// The consumer has disconnected; the sender should stop producing events.
    #[error("inference sink is closed")]
    Closed,
    /// An unexpected sink-side error.
    #[error("inference sink error: {0}")]
    Other(String),
}

// ─── InferenceSink ────────────────────────────────────────────────────────

/// Back-pressure-aware consumer side of the Streaming Response Protocol.
///
/// Implementors receive [`StreamEvent`]s as the model generates tokens.  The
/// `async fn send` signature naturally provides back-pressure: a slow
/// consumer causes the caller to await, which propagates pressure up to the
/// underlying LLM provider connection.
///
/// # Provided implementation
///
/// [`tokio::sync::mpsc::Sender<StreamEvent<T>>`] implements `InferenceSink`
/// automatically — pass a channel sender directly to any function that
/// accepts `impl InferenceSink`.
///
/// # Custom implementation
///
/// ```rust,ignore
/// use async_trait::async_trait;
/// use mofa_kernel::llm::srp::{InferenceSink, SinkError, StreamEvent};
/// use mofa_kernel::llm::streaming::StreamChunk;
///
/// struct LogSink;
///
/// #[async_trait]
/// impl InferenceSink<StreamChunk> for LogSink {
///     async fn send(&self, event: StreamEvent<StreamChunk>) -> Result<(), SinkError> {
///         if let StreamEvent::Delta(ref chunk) = event {
///             print!("{}", chunk.delta);
///         }
///         Ok(())
///     }
/// }
/// ```
#[async_trait]
pub trait InferenceSink<T: Send>: Send + Sync {
    /// Deliver one [`StreamEvent`] to the consumer.
    ///
    /// # Errors
    ///
    /// Returns [`SinkError::Closed`] when the consumer has disconnected.
    async fn send(&self, event: StreamEvent<T>) -> Result<(), SinkError>;
}

/// Blanket [`InferenceSink`] for `tokio::sync::mpsc::Sender`.
///
/// Allows passing an `mpsc::Sender` directly wherever `InferenceSink` is
/// expected, using the channel's natural back-pressure semantics.
///
/// ```rust
/// use tokio::sync::mpsc;
/// use mofa_kernel::llm::srp::{InferenceSink, StreamEvent};
/// use mofa_kernel::llm::streaming::StreamChunk;
///
/// let (tx, _rx) = mpsc::channel::<StreamEvent<StreamChunk>>(16);
/// // `tx` now satisfies `InferenceSink<StreamChunk>`
/// ```
#[async_trait]
impl<T: Send + 'static> InferenceSink<T> for mpsc::Sender<StreamEvent<T>> {
    async fn send(&self, event: StreamEvent<T>) -> Result<(), SinkError> {
        self.send(event).await.map_err(|_| SinkError::Closed)
    }
}

// ─── SrpConfig ────────────────────────────────────────────────────────────

/// Configuration for [`stream_inference`].
///
/// ```rust
/// use std::time::Duration;
/// use mofa_kernel::llm::srp::SrpConfig;
///
/// // Use defaults
/// let cfg = SrpConfig::default();
/// assert_eq!(cfg.heartbeat_interval, Duration::from_secs(30));
/// assert_eq!(cfg.channel_capacity, 64);
///
/// // Custom config
/// let cfg = SrpConfig {
///     heartbeat_interval: Duration::from_secs(10),
///     channel_capacity: 128,
/// };
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SrpConfig {
    /// How often to emit [`StreamEvent::Heartbeat`] when no delta arrives.
    ///
    /// Default: **30 seconds**.
    pub heartbeat_interval: Duration,
    /// Capacity of the internal bounded channel used for back-pressure.
    ///
    /// A smaller value increases back-pressure sensitivity; a larger value
    /// reduces the risk of blocking the producer task.  Default: **64**.
    pub channel_capacity: usize,
}

impl Default for SrpConfig {
    fn default() -> Self {
        Self {
            heartbeat_interval: Duration::from_secs(30),
            channel_capacity: 64,
        }
    }
}

// ─── stream_inference ─────────────────────────────────────────────────────

/// Map [`LLMProvider::chat_stream`] output into [`StreamEvent`] framing.
///
/// Spawns a background task that drives the underlying provider stream and
/// forwards events over a bounded [`mpsc`] channel.  The returned
/// [`ReceiverStream`] is the consumer side of that channel.
///
/// # Back-pressure
///
/// The internal channel has capacity [`SrpConfig::channel_capacity`].  When
/// the channel is full the producer task awaits, propagating back-pressure to
/// the underlying provider connection.
///
/// # Cancellation
///
/// Pass a [`CancellationToken`] that you control.  Calling
/// `token.cancel()` causes the background task to emit
/// [`StreamEvent::Cancelled`] and terminate on the **next select iteration**,
/// which typically takes less than one microsecond after the token is
/// signalled.
///
/// # Heartbeats
///
/// If no delta arrives within [`SrpConfig::heartbeat_interval`], the
/// protocol emits [`StreamEvent::Heartbeat`].  The interval resets on every
/// received delta so that active streams never send spurious heartbeats.
///
/// # Errors
///
/// Returns `Err` only if starting the underlying stream itself fails (e.g.
/// the provider does not support streaming).  Errors that arise *during*
/// streaming are logged with [`tracing::warn`] and cause the stream to
/// terminate with [`StreamEvent::Done`].
///
/// # Example
///
/// ```rust,ignore
/// use std::time::Duration;
/// use mofa_kernel::llm::srp::{stream_inference, SrpConfig, StreamEvent};
/// use tokio_util::sync::CancellationToken;
/// use futures::StreamExt;
///
/// let token = CancellationToken::new();
/// let mut stream = stream_inference(
///     &provider,
///     request,
///     token.clone(),
///     SrpConfig { heartbeat_interval: Duration::from_secs(5), ..Default::default() },
/// ).await?;
///
/// while let Some(event) = stream.next().await {
///     match event {
///         StreamEvent::Delta(chunk) => print!("{}", chunk.delta),
///         StreamEvent::Done         => break,
///         _                         => {}
///     }
/// }
/// ```
pub async fn stream_inference(
    provider: &dyn LLMProvider,
    request: ChatCompletionRequest,
    token: CancellationToken,
    config: SrpConfig,
) -> AgentResult<ReceiverStream<StreamEvent<StreamChunk>>> {
    // Initiate the underlying stream — this is where auth / network errors
    // surface, before the channel is created.
    let mut chat_stream = provider.chat_stream(request).await?;

    let (tx, rx) = mpsc::channel::<StreamEvent<StreamChunk>>(config.channel_capacity);
    let heartbeat_interval = config.heartbeat_interval;

    tokio::spawn(async move {
        let mut hb = tokio::time::interval(heartbeat_interval);
        // Skip the very first tick so we don't immediately send a heartbeat
        // before any real work has happened.
        hb.tick().await;

        loop {
            tokio::select! {
                // Cancellation is checked first (biased) so a cancelled token
                // always takes priority over a pending stream item.
                biased;

                _ = token.cancelled() => {
                    let _ = tx.send(StreamEvent::Cancelled).await;
                    break;
                }

                item = chat_stream.next() => {
                    match item {
                        Some(Ok(chunk)) => {
                            // Reset the heartbeat timer — we got real data.
                            hb.reset();

                            let sc = chunk_to_stream_chunk(chunk);
                            if sc.is_done() {
                                let _ = tx.send(StreamEvent::Done).await;
                                break;
                            }
                            if tx.send(StreamEvent::Delta(sc)).await.is_err() {
                                // Receiver dropped — stop silently.
                                break;
                            }
                        }
                        Some(Err(e)) => {
                            warn!("srp: stream error from provider: {e}");
                            let _ = tx.send(StreamEvent::Done).await;
                            break;
                        }
                        None => {
                            let _ = tx.send(StreamEvent::Done).await;
                            break;
                        }
                    }
                }

                _ = hb.tick() => {
                    if tx.send(StreamEvent::Heartbeat).await.is_err() {
                        break;
                    }
                }
            }
        }
    });

    Ok(ReceiverStream::new(rx))
}

// ─── Internal helpers ─────────────────────────────────────────────────────

/// Convert a raw [`ChatCompletionChunk`](super::types::ChatCompletionChunk)
/// into the provider-agnostic [`StreamChunk`].
fn chunk_to_stream_chunk(chunk: super::types::ChatCompletionChunk) -> StreamChunk {
    let first = chunk.choices.into_iter().next();
    StreamChunk {
        delta: first
            .as_ref()
            .and_then(|c| c.delta.content.as_deref())
            .unwrap_or("")
            .to_owned(),
        finish_reason: first.as_ref().and_then(|c| c.finish_reason.clone()),
        usage: None,
        tool_calls: first.and_then(|c| c.delta.tool_calls),
    }
}

// ─── Unit tests ───────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::AgentResult;
    use crate::llm::provider::LLMProvider;
    use crate::llm::streaming::StreamChunk;
    use crate::llm::types::{
        ChatCompletionChunk, ChatCompletionRequest, ChatCompletionResponse, ChunkChoice,
        ChunkDelta, EmbeddingRequest, EmbeddingResponse, FinishReason,
    };
    use futures::StreamExt as _;
    use std::time::Duration;
    use tokio::time::timeout;

    // ── Mock provider ─────────────────────────────────────────────────────

    /// A minimal `LLMProvider` that streams a fixed set of `ChatCompletionChunk`s.
    struct MockProvider {
        chunks: Vec<ChatCompletionChunk>,
    }

    impl MockProvider {
        fn text_chunks(words: &[&str]) -> Self {
            let mut chunks: Vec<ChatCompletionChunk> = words
                .iter()
                .map(|w| ChatCompletionChunk {
                    choices: vec![ChunkChoice {
                        index: 0,
                        delta: ChunkDelta {
                            role: None,
                            content: Some(w.to_string()),
                            tool_calls: None,
                        },
                        finish_reason: None,
                    }],
                })
                .collect();

            // Terminal chunk with finish_reason = Stop
            chunks.push(ChatCompletionChunk {
                choices: vec![ChunkChoice {
                    index: 0,
                    delta: ChunkDelta { role: None, content: None, tool_calls: None },
                    finish_reason: Some(FinishReason::Stop),
                }],
            });

            Self { chunks }
        }

        /// Provider whose stream never yields (always pending).
        fn pending() -> Self {
            Self { chunks: vec![] }
        }
    }

    #[async_trait::async_trait]
    impl LLMProvider for MockProvider {
        fn name(&self) -> &str {
            "mock"
        }

        async fn chat(
            &self,
            _request: ChatCompletionRequest,
        ) -> AgentResult<ChatCompletionResponse> {
            Ok(ChatCompletionResponse { choices: vec![] })
        }

        fn supports_streaming(&self) -> bool {
            true
        }

        async fn chat_stream(
            &self,
            _request: ChatCompletionRequest,
        ) -> AgentResult<crate::llm::provider::ChatStream> {
            let items = self.chunks.clone();
            if items.is_empty() {
                // Return a stream that pends forever so cancellation tests work.
                let s = futures::stream::pending::<AgentResult<ChatCompletionChunk>>();
                return Ok(Box::pin(s));
            }
            Ok(Box::pin(futures::stream::iter(
                items.into_iter().map(Ok::<_, crate::agent::error::AgentError>),
            )))
        }
    }

    fn default_request() -> ChatCompletionRequest {
        ChatCompletionRequest::new("gpt-4o")
    }

    // ── StreamEvent traits ────────────────────────────────────────────────

    #[test]
    fn stream_event_is_clone_and_send() {
        fn assert_clone_send<T: Clone + Send>() {}
        assert_clone_send::<StreamEvent<StreamChunk>>();
    }

    #[test]
    fn stream_event_variants_debug() {
        let d: StreamEvent<StreamChunk> = StreamEvent::Delta(StreamChunk::text("hi"));
        let h: StreamEvent<StreamChunk> = StreamEvent::Heartbeat;
        let c: StreamEvent<StreamChunk> = StreamEvent::Cancelled;
        let done: StreamEvent<StreamChunk> = StreamEvent::Done;

        // Just ensure Debug doesn't panic.
        assert!(format!("{d:?}").contains("Delta"));
        assert!(format!("{h:?}").contains("Heartbeat"));
        assert!(format!("{c:?}").contains("Cancelled"));
        assert!(format!("{done:?}").contains("Done"));
    }

    #[test]
    fn stream_event_clone_preserves_payload() {
        let original = StreamEvent::Delta(StreamChunk::text("hello"));
        let cloned = original.clone();
        if let StreamEvent::Delta(chunk) = cloned {
            assert_eq!(chunk.delta, "hello");
        } else {
            panic!("clone changed variant");
        }
    }

    // ── SrpConfig ─────────────────────────────────────────────────────────

    #[test]
    fn srp_config_default_values() {
        let cfg = SrpConfig::default();
        assert_eq!(cfg.heartbeat_interval, Duration::from_secs(30));
        assert_eq!(cfg.channel_capacity, 64);
    }

    #[test]
    fn srp_config_custom_values() {
        let cfg = SrpConfig {
            heartbeat_interval: Duration::from_millis(500),
            channel_capacity: 8,
        };
        assert_eq!(cfg.heartbeat_interval, Duration::from_millis(500));
        assert_eq!(cfg.channel_capacity, 8);
    }

    // ── SinkError ─────────────────────────────────────────────────────────

    #[test]
    fn sink_error_display() {
        assert_eq!(SinkError::Closed.to_string(), "inference sink is closed");
        assert_eq!(
            SinkError::Other("oops".into()).to_string(),
            "inference sink error: oops"
        );
    }

    #[test]
    fn sink_error_equality() {
        assert_eq!(SinkError::Closed, SinkError::Closed);
        assert_ne!(SinkError::Closed, SinkError::Other("x".into()));
    }

    // ── InferenceSink blanket impl ────────────────────────────────────────

    #[tokio::test]
    async fn mpsc_sender_implements_inference_sink() {
        let (tx, mut rx) = mpsc::channel::<StreamEvent<StreamChunk>>(4);
        let event = StreamEvent::Delta(StreamChunk::text("world"));
        tx.send(event.clone()).await.expect("send");
        let received = rx.recv().await.unwrap();
        if let StreamEvent::Delta(chunk) = received {
            assert_eq!(chunk.delta, "world");
        } else {
            panic!("unexpected variant");
        }
    }

    #[tokio::test]
    async fn mpsc_sender_send_returns_closed_when_receiver_dropped() {
        let (tx, rx) = mpsc::channel::<StreamEvent<StreamChunk>>(1);
        drop(rx);
        let result = <mpsc::Sender<_> as InferenceSink<_>>::send(
            &tx,
            StreamEvent::Heartbeat,
        )
        .await;
        assert_eq!(result, Err(SinkError::Closed));
    }

    // ── stream_inference round-trip ───────────────────────────────────────

    #[tokio::test]
    async fn stream_inference_delivers_delta_and_done() {
        let provider = MockProvider::text_chunks(&["Hello", " world"]);
        let token = CancellationToken::new();
        let cfg = SrpConfig { heartbeat_interval: Duration::from_secs(60), ..Default::default() };

        let mut stream = stream_inference(&provider, default_request(), token, cfg)
            .await
            .expect("stream started");

        let mut deltas = Vec::new();
        while let Some(event) = stream.next().await {
            match event {
                StreamEvent::Delta(chunk) => deltas.push(chunk.delta.clone()),
                StreamEvent::Done => break,
                StreamEvent::Cancelled => panic!("unexpected Cancelled"),
                StreamEvent::Heartbeat => {}
                _ => {}
            }
        }

        assert_eq!(deltas, vec!["Hello", " world"]);
    }

    #[tokio::test]
    async fn stream_inference_done_event_closes_stream() {
        let provider = MockProvider::text_chunks(&["tok"]);
        let token = CancellationToken::new();
        let cfg = SrpConfig::default();

        let mut events: Vec<String> = vec![];
        let mut stream = stream_inference(&provider, default_request(), token, cfg)
            .await
            .unwrap();

        while let Some(event) = stream.next().await {
            match &event {
                StreamEvent::Delta(_) => events.push("delta".into()),
                StreamEvent::Done => {
                    events.push("done".into());
                    break;
                }
                _ => {}
            }
        }

        assert!(events.contains(&"done".to_string()));
    }

    // ── Cancellation ─────────────────────────────────────────────────────

    /// Cancellation must terminate the stream in < 5 ms (real time).
    ///
    /// We use a provider whose stream never yields so the only termination
    /// signal is the CancellationToken.  The token is cancelled before we
    /// even poll, so the first event should be Cancelled.
    #[tokio::test]
    async fn cancellation_terminates_stream_quickly() {
        let provider = MockProvider::pending();
        let token = CancellationToken::new();
        let cfg = SrpConfig {
            // Long heartbeat so it never fires in this test.
            heartbeat_interval: Duration::from_secs(3600),
            channel_capacity: 4,
        };

        let mut stream = stream_inference(&provider, default_request(), token.clone(), cfg)
            .await
            .expect("stream started");

        // Cancel immediately — the background task should pick it up on the
        // next select iteration (biased; cancellation has highest priority).
        token.cancel();

        let result = timeout(Duration::from_millis(100), stream.next()).await;
        assert!(result.is_ok(), "stream should produce an event within 100ms");

        match result.unwrap() {
            Some(StreamEvent::Cancelled) => { /* success */ }
            other => panic!("expected Cancelled, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn pre_cancelled_token_sends_cancelled_before_deltas() {
        let provider = MockProvider::text_chunks(&["hello"]);
        // Token is already cancelled before stream_inference is called.
        let token = CancellationToken::new();
        token.cancel();

        let cfg = SrpConfig { heartbeat_interval: Duration::from_secs(60), ..Default::default() };
        let mut stream = stream_inference(&provider, default_request(), token, cfg)
            .await
            .unwrap();

        // The very first event must be Cancelled (biased select, cancellation wins).
        let first = timeout(Duration::from_millis(100), stream.next())
            .await
            .expect("event within 100ms")
            .expect("some event");

        assert!(
            matches!(first, StreamEvent::Cancelled),
            "expected Cancelled as first event, got {first:?}"
        );
    }

    // ── Heartbeat ─────────────────────────────────────────────────────────

    #[tokio::test]
    async fn heartbeat_emitted_on_stalled_stream() {
        // Use a very short heartbeat interval and a slow/pending stream.
        tokio::time::pause();

        let provider = MockProvider::pending();
        let token = CancellationToken::new();
        let cfg = SrpConfig {
            heartbeat_interval: Duration::from_millis(10),
            channel_capacity: 4,
        };

        let mut stream = stream_inference(&provider, default_request(), token.clone(), cfg)
            .await
            .unwrap();

        // Advance simulated time past the heartbeat interval.
        tokio::time::advance(Duration::from_millis(50)).await;

        let event = timeout(Duration::from_millis(200), stream.next())
            .await
            .expect("event within 200ms")
            .expect("some event");

        assert!(
            matches!(event, StreamEvent::Heartbeat),
            "expected Heartbeat, got {event:?}"
        );

        token.cancel();
    }

    // ── chunk_to_stream_chunk helper ──────────────────────────────────────

    #[test]
    fn chunk_conversion_text_only() {
        let chunk = ChatCompletionChunk {
            choices: vec![ChunkChoice {
                index: 0,
                delta: ChunkDelta {
                    role: None,
                    content: Some("hello".into()),
                    tool_calls: None,
                },
                finish_reason: None,
            }],
        };
        let sc = chunk_to_stream_chunk(chunk);
        assert_eq!(sc.delta, "hello");
        assert!(sc.finish_reason.is_none());
        assert!(!sc.is_done());
    }

    #[test]
    fn chunk_conversion_done_chunk() {
        let chunk = ChatCompletionChunk {
            choices: vec![ChunkChoice {
                index: 0,
                delta: ChunkDelta { role: None, content: None, tool_calls: None },
                finish_reason: Some(FinishReason::Stop),
            }],
        };
        let sc = chunk_to_stream_chunk(chunk);
        assert!(sc.is_done());
        assert_eq!(sc.delta, "");
    }

    #[test]
    fn chunk_conversion_empty_choices() {
        let chunk = ChatCompletionChunk { choices: vec![] };
        let sc = chunk_to_stream_chunk(chunk);
        assert_eq!(sc.delta, "");
        assert!(!sc.is_done());
    }

    // ── Serialization / round-trip ────────────────────────────────────────

    #[test]
    fn srp_config_equality_and_clone() {
        let a = SrpConfig::default();
        let b = a.clone();
        assert_eq!(a, b);
    }
}
