//! Mock LLM provider for `mofa-foundation` integration tests.
//!
//! [`MockLLMProvider`] implements [`mofa_kernel::llm::LLMProvider`] and is the
//! canonical test double for all inference end-to-end tests in this crate.
//! It records every call, returns configurable canned responses queued at
//! construction time, and falls back to sensible defaults when the queue is
//! empty.
//!
//! # Design goals
//!
//! | Goal | Mechanism |
//! |------|-----------|
//! | Deterministic | Responses queued at build time; no random state |
//! | Observable | `chat_call_count()`, `last_chat_request()`, … |
//! | Composable | Builder adds responses one-by-one in FIFO order |
//! | Thread-safe | Internal state protected by `Arc<Mutex<…>>` |
//! | Explicit errors | `respond_with_error()` forces typed `AgentError` |
//!
//! # Example
//!
//! ```rust,ignore
//! use crate::common::mock_provider::MockLLMProvider;
//! use mofa_kernel::llm::{ChatCompletionRequest, LLMProvider};
//!
//! let mock = MockLLMProvider::builder()
//!     .respond_with("Pong!")
//!     .build();
//!
//! let request = ChatCompletionRequest::new("mock-model").user("Ping");
//! let response = tokio_test::block_on(mock.chat(request)).unwrap();
//! assert_eq!(response.content(), Some("Pong!"));
//! assert_eq!(mock.chat_call_count(), 1);
//! ```

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use futures::stream;

use mofa_kernel::agent::{AgentError, AgentResult};
use mofa_kernel::llm::{
    ChatCompletionChunk, ChatCompletionRequest, ChatCompletionResponse, ChatMessage, ChatStream,
    Choice, ChunkChoice, ChunkDelta, EmbeddingData, EmbeddingRequest, EmbeddingResponse,
    FinishReason, LLMProvider,
};

// ─────────────────────────────────────────────────────────────────────────────
// Internal mutable state
// ─────────────────────────────────────────────────────────────────────────────

/// All recorded calls and queued responses belonging to one [`MockLLMProvider`].
///
/// Protected behind `Arc<Mutex<…>>` so the `&self` async methods of
/// [`LLMProvider`] can mutate it without `&mut self`.
struct MockState {
    /// Every [`ChatCompletionRequest`] passed to [`LLMProvider::chat`], in call order.
    chat_calls: Vec<ChatCompletionRequest>,
    /// Every [`ChatCompletionRequest`] passed to [`LLMProvider::chat_stream`], in call order.
    stream_calls: Vec<ChatCompletionRequest>,
    /// Every [`EmbeddingRequest`] passed to [`LLMProvider::embedding`], in call order.
    embedding_calls: Vec<EmbeddingRequest>,
    /// Number of invocations of [`LLMProvider::health_check`].
    health_check_calls: u32,

    /// FIFO queue of responses returned by [`LLMProvider::chat`].
    /// When empty, a default "mock response" string is returned.
    chat_responses: VecDeque<AgentResult<ChatCompletionResponse>>,
    /// FIFO queue of chunk sequences for [`LLMProvider::chat_stream`].
    ///
    /// Each inner `Vec` is one complete stream (one per `chat_stream()` call).
    /// Within the inner `Vec` each element is one SSE chunk.
    stream_sequences: VecDeque<Vec<AgentResult<ChatCompletionChunk>>>,
    /// FIFO queue of responses for [`LLMProvider::embedding`].
    embedding_responses: VecDeque<AgentResult<EmbeddingResponse>>,
    /// FIFO queue of results for [`LLMProvider::health_check`].
    health_responses: VecDeque<AgentResult<bool>>,
}

impl Default for MockState {
    fn default() -> Self {
        Self {
            chat_calls: Vec::new(),
            stream_calls: Vec::new(),
            embedding_calls: Vec::new(),
            health_check_calls: 0,
            chat_responses: VecDeque::new(),
            stream_sequences: VecDeque::new(),
            embedding_responses: VecDeque::new(),
            health_responses: VecDeque::new(),
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Public type
// ─────────────────────────────────────────────────────────────────────────────

/// A deterministic, in-process mock of [`LLMProvider`].
///
/// Construct one with [`MockLLMProvider::builder()`].
///
/// The provider is thread-safe: it can be wrapped in `Arc` and shared across
/// async tasks.  All mutation is guarded by an internal [`Mutex`]; no await
/// point is held while the lock is taken.
pub struct MockLLMProvider {
    /// Returned by [`LLMProvider::name`].
    name: String,
    /// Returned by [`LLMProvider::default_model`].
    default_model: String,
    /// Controls [`LLMProvider::supports_streaming`].
    supports_streaming: bool,
    /// Controls [`LLMProvider::supports_embedding`].
    supports_embedding: bool,
    /// Controls [`LLMProvider::supports_tools`].
    supports_tools: bool,
    /// Controls [`LLMProvider::supports_vision`].
    supports_vision: bool,
    /// Thread-safe mutable call recorder and response queue.
    state: Arc<Mutex<MockState>>,
}

impl MockLLMProvider {
    /// Begin building a [`MockLLMProvider`] with sensible defaults:
    /// - `name`: `"mock-provider"`
    /// - `default_model`: `"mock-model"`
    /// - `supports_streaming`: `true`
    /// - `supports_embedding`: `false` (common in real providers)
    /// - `supports_tools`: `true`
    /// - `supports_vision`: `false`
    pub fn builder() -> MockLLMProviderBuilder {
        MockLLMProviderBuilder::default()
    }

    // ── Observation helpers ──────────────────────────────────────────────────

    /// Total number of [`LLMProvider::chat`] calls received.
    pub fn chat_call_count(&self) -> usize {
        self.state.lock().expect("mock state mutex poisoned").chat_calls.len()
    }

    /// Returns a clone of every [`ChatCompletionRequest`] passed to
    /// [`LLMProvider::chat`], in call order.
    pub fn chat_calls(&self) -> Vec<ChatCompletionRequest> {
        self.state.lock().expect("mock state mutex poisoned").chat_calls.clone()
    }

    /// The most recent request received by [`LLMProvider::chat`], or `None`
    /// if the method has never been called.
    pub fn last_chat_request(&self) -> Option<ChatCompletionRequest> {
        self.state
            .lock()
            .expect("mock state mutex poisoned")
            .chat_calls
            .last()
            .cloned()
    }

    /// Total number of [`LLMProvider::chat_stream`] calls received.
    pub fn stream_call_count(&self) -> usize {
        self.state.lock().expect("mock state mutex poisoned").stream_calls.len()
    }

    /// Total number of [`LLMProvider::embedding`] calls received.
    pub fn embedding_call_count(&self) -> usize {
        self.state.lock().expect("mock state mutex poisoned").embedding_calls.len()
    }

    /// Total number of [`LLMProvider::health_check`] calls received.
    pub fn health_check_call_count(&self) -> u32 {
        self.state.lock().expect("mock state mutex poisoned").health_check_calls
    }

    /// `true` if *any* method on this provider has been invoked at least once.
    pub fn was_called(&self) -> bool {
        let s = self.state.lock().expect("mock state mutex poisoned");
        !s.chat_calls.is_empty()
            || !s.stream_calls.is_empty()
            || !s.embedding_calls.is_empty()
            || s.health_check_calls > 0
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// LLMProvider implementation
// ─────────────────────────────────────────────────────────────────────────────

#[async_trait]
impl LLMProvider for MockLLMProvider {
    fn name(&self) -> &str {
        &self.name
    }

    fn default_model(&self) -> &str {
        &self.default_model
    }

    fn supported_models(&self) -> Vec<&str> {
        vec![self.default_model.as_str()]
    }

    fn supports_streaming(&self) -> bool {
        self.supports_streaming
    }

    fn supports_tools(&self) -> bool {
        self.supports_tools
    }

    fn supports_vision(&self) -> bool {
        self.supports_vision
    }

    fn supports_embedding(&self) -> bool {
        self.supports_embedding
    }

    /// Records the request, then pops the front of the response queue.
    ///
    /// When the queue is exhausted the default canned response
    /// `"This is a mock response."` is returned.
    async fn chat(&self, request: ChatCompletionRequest) -> AgentResult<ChatCompletionResponse> {
        let mut state = self.state.lock().expect("mock state mutex poisoned");
        state.chat_calls.push(request);
        if let Some(queued) = state.chat_responses.pop_front() {
            queued
        } else {
            Ok(make_default_chat_response("This is a mock response."))
        }
    }

    /// Opens a streaming response.
    ///
    /// Returns [`AgentError::Other`] immediately (before the stream is opened)
    /// when `supports_streaming` is `false`.
    ///
    /// Otherwise records the request, pops the front chunk sequence, and wraps
    /// it in a `futures::stream::iter` so the caller can `.next().await` over it.
    ///
    /// When the sequence queue is empty a single-token default stream is used.
    async fn chat_stream(&self, request: ChatCompletionRequest) -> AgentResult<ChatStream> {
        if !self.supports_streaming {
            return Err(AgentError::Other(format!(
                "Provider \"{}\" does not support streaming",
                self.name
            )));
        }

        let chunks = {
            let mut state = self.state.lock().expect("mock state mutex poisoned");
            state.stream_calls.push(request);
            if let Some(seq) = state.stream_sequences.pop_front() {
                seq
            } else {
                build_stream_sequence(vec!["mock stream token".to_string()])
            }
            // MutexGuard drops here — no await is held across the lock.
        };

        Ok(Box::pin(stream::iter(chunks)))
    }

    /// Returns [`AgentError::Other`] when `supports_embedding` is `false`.
    ///
    /// Otherwise records the request, pops the front of the embedding response
    /// queue, and falls back to a zero-vector sentinel when the queue is empty.
    async fn embedding(&self, request: EmbeddingRequest) -> AgentResult<EmbeddingResponse> {
        if !self.supports_embedding {
            return Err(AgentError::Other(format!(
                "Provider \"{}\" does not support embedding",
                self.name
            )));
        }

        let mut state = self.state.lock().expect("mock state mutex poisoned");
        state.embedding_calls.push(request);
        if let Some(queued) = state.embedding_responses.pop_front() {
            queued
        } else {
            Ok(make_default_embedding_response())
        }
    }

    /// Counts the call, pops the front of the health-response queue, and
    /// defaults to `Ok(true)` when no response is queued.
    async fn health_check(&self) -> AgentResult<bool> {
        let mut state = self.state.lock().expect("mock state mutex poisoned");
        state.health_check_calls += 1;
        if let Some(queued) = state.health_responses.pop_front() {
            queued
        } else {
            Ok(true)
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Builder
// ─────────────────────────────────────────────────────────────────────────────

/// Fluent builder for [`MockLLMProvider`].
///
/// Obtain one via [`MockLLMProvider::builder()`].
pub struct MockLLMProviderBuilder {
    name: String,
    default_model: String,
    supports_streaming: bool,
    supports_embedding: bool,
    supports_tools: bool,
    supports_vision: bool,
    state: MockState,
}

impl Default for MockLLMProviderBuilder {
    fn default() -> Self {
        Self {
            name: "mock-provider".to_string(),
            default_model: "mock-model".to_string(),
            supports_streaming: true,
            supports_embedding: false,
            supports_tools: true,
            supports_vision: false,
            state: MockState::default(),
        }
    }
}

impl MockLLMProviderBuilder {
    // ── Capability flags ────────────────────────────────────────────────────

    /// Override the provider name (default: `"mock-provider"`).
    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.name = name.into();
        self
    }

    /// Override the default model name (default: `"mock-model"`).
    pub fn with_default_model(mut self, model: impl Into<String>) -> Self {
        self.default_model = model.into();
        self
    }

    /// Enable or disable streaming (default: `true`).
    pub fn with_streaming(mut self, enabled: bool) -> Self {
        self.supports_streaming = enabled;
        self
    }

    /// Enable or disable embedding (default: `false`).
    pub fn with_embedding(mut self, enabled: bool) -> Self {
        self.supports_embedding = enabled;
        self
    }

    /// Enable or disable tool calling (default: `true`).
    pub fn with_tools(mut self, enabled: bool) -> Self {
        self.supports_tools = enabled;
        self
    }

    /// Enable or disable vision (default: `false`).
    pub fn with_vision(mut self, enabled: bool) -> Self {
        self.supports_vision = enabled;
        self
    }

    // ── Response queue helpers ───────────────────────────────────────────────

    /// Enqueue a successful chat response whose text content is `content`.
    ///
    /// Calls to [`LLMProvider::chat`] consume these in FIFO order; the default
    /// `"This is a mock response."` fires once the queue is empty.
    pub fn respond_with(mut self, content: impl Into<String>) -> Self {
        self.state
            .chat_responses
            .push_back(Ok(make_default_chat_response(content.into())));
        self
    }

    /// Enqueue an error response for the next [`LLMProvider::chat`] call.
    pub fn respond_with_error(mut self, error: AgentError) -> Self {
        self.state.chat_responses.push_back(Err(error));
        self
    }

    /// Enqueue a streaming sequence.
    ///
    /// Each element of `tokens` becomes one `content`-carrying `Delta` chunk;
    /// a terminal empty `Stop` chunk is automatically appended.
    ///
    /// Sequences are consumed in FIFO order per [`LLMProvider::chat_stream`]
    /// call.
    pub fn stream_with_tokens(mut self, tokens: Vec<impl Into<String>>) -> Self {
        self.state.stream_sequences.push_back(build_stream_sequence(tokens));
        self
    }

    /// Enqueue a single-item error stream for the next
    /// [`LLMProvider::chat_stream`] call.
    ///
    /// The stream opens successfully (`Ok(stream)`) but the first item yielded
    /// by the stream is `Err(error)`.
    pub fn stream_with_error(mut self, error: AgentError) -> Self {
        self.state.stream_sequences.push_back(vec![Err(error)]);
        self
    }

    /// Enqueue a successful embedding response containing `embedding`.
    ///
    /// Requires [`with_embedding(true)`] to be set; otherwise the provider
    /// returns an error before consulting the queue.
    pub fn embedding_responds_with(mut self, embedding: Vec<f32>) -> Self {
        self.state.embedding_responses.push_back(Ok(EmbeddingResponse {
            data: vec![EmbeddingData {
                object: "embedding".to_string(),
                index: 0,
                embedding,
            }],
            usage: None,
        }));
        self
    }

    /// Enqueue a health-check result.
    ///
    /// Calls to [`LLMProvider::health_check`] consume these in FIFO order;
    /// `Ok(true)` fires once the queue is empty.
    pub fn health_responds_with(mut self, result: AgentResult<bool>) -> Self {
        self.state.health_responses.push_back(result);
        self
    }

    /// Finalise the builder and return a [`MockLLMProvider`].
    pub fn build(self) -> MockLLMProvider {
        MockLLMProvider {
            name: self.name,
            default_model: self.default_model,
            supports_streaming: self.supports_streaming,
            supports_embedding: self.supports_embedding,
            supports_tools: self.supports_tools,
            supports_vision: self.supports_vision,
            state: Arc::new(Mutex::new(self.state)),
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Private helpers
// ─────────────────────────────────────────────────────────────────────────────

/// Build a minimal [`ChatCompletionResponse`] with a single assistant message.
fn make_default_chat_response(content: impl Into<String>) -> ChatCompletionResponse {
    ChatCompletionResponse {
        choices: vec![Choice {
            index: 0,
            message: ChatMessage::assistant(content.into()),
            finish_reason: Some(FinishReason::Stop),
            logprobs: None,
        }],
    }
}

/// Convert `tokens` into a `Vec<AgentResult<ChatCompletionChunk>>`.
///
/// Each token becomes one `content`-filled `Delta` chunk; a final empty `Stop`
/// chunk is appended to signal end-of-stream to the consumer.
fn build_stream_sequence<S: Into<String>>(
    tokens: Vec<S>,
) -> Vec<AgentResult<ChatCompletionChunk>> {
    let token_count = tokens.len();
    let mut seq: Vec<AgentResult<ChatCompletionChunk>> = tokens
        .into_iter()
        .enumerate()
        .map(|(idx, token)| {
            Ok(ChatCompletionChunk {
                choices: vec![ChunkChoice {
                    index: idx as u32,
                    delta: ChunkDelta {
                        role: None,
                        content: Some(token.into()),
                        tool_calls: None,
                    },
                    finish_reason: None,
                }],
            })
        })
        .collect();

    // Terminal stop chunk — signals end-of-stream.
    seq.push(Ok(ChatCompletionChunk {
        choices: vec![ChunkChoice {
            index: token_count as u32,
            delta: ChunkDelta::default(),
            finish_reason: Some(FinishReason::Stop),
        }],
    }));

    seq
}

/// Build a minimal embedding response containing a 4-element zero-vector.
///
/// Used when the embedding-response queue is empty (fallback sentinel).
fn make_default_embedding_response() -> EmbeddingResponse {
    EmbeddingResponse {
        data: vec![EmbeddingData {
            object: "embedding".to_string(),
            index: 0,
            embedding: vec![0.0_f32; 4],
        }],
        usage: None,
    }
}
