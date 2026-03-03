//! Inference Request Protocol (IRP)
//!
//! The IRP formalises a backend-agnostic, unified envelope for every kind of
//! LLM inference call — analogous to "HTTP method + URL + headers" for
//! language-model interactions.
//!
//! # Components
//!
//! | Type | Role |
//! |------|------|
//! | [`InferenceRequest`]      | Unified request envelope: text / multimodal / tool-call / embedding |
//! | [`InferenceResponse`]     | Unified synchronous response |
//! | [`InferenceCapabilities`] | Capability advertisement returned by a backend |
//! | [`InferenceProtocol`]     | Trait with blanket defaults for optional capabilities |
//!
//! # Quick start
//!
//! ```rust
//! use mofa_kernel::llm::irp::{InferenceCapabilities, InferenceRequest, RequestModality};
//!
//! // Build a plain-text request
//! let req = InferenceRequest::text("gpt-4o", "What is the IRP?");
//! assert_eq!(req.model, "gpt-4o");
//! assert!(!req.stream);
//!
//! // Build a text request with options (validation is explicit)
//! let req = InferenceRequest::text("gpt-4o", "Explain caching")
//!     .with_temperature(0.7)
//!     .expect("temperature in range")
//!     .with_max_tokens(512)
//!     .expect("max_tokens > 0");
//! assert_eq!(req.temperature, Some(0.7));
//!
//! // Inspect capabilities
//! let caps = InferenceCapabilities {
//!     streaming: true,
//!     tool_calling: true,
//!     ..Default::default()
//! };
//! assert!(caps.supports_modality(&RequestModality::Text));
//! assert!(!caps.supports_modality(&RequestModality::Multimodal));
//! ```

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::agent::AgentResult;

use super::types::{
    ChatCompletionRequest, ChatCompletionResponse, ChatMessage, EmbeddingInput, EmbeddingRequest,
    EmbeddingResponse, Tool, ToolChoice,
};

// ─── Request modality ──────────────────────────────────────────────────────

/// Describes which inference modality an [`InferenceRequest`] carries.
///
/// The enum is `#[non_exhaustive]` so that new modalities (e.g. audio
/// generation, video understanding) can be added in future releases without
/// breaking existing `match` arms in downstream code.
///
/// # Matching
///
/// Always include a catch-all when matching on this enum:
///
/// ```rust
/// use mofa_kernel::llm::irp::RequestModality;
///
/// fn describe(m: &RequestModality) -> &'static str {
///     match m {
///         RequestModality::Text       => "text chat",
///         RequestModality::Multimodal => "multimodal",
///         RequestModality::ToolCall   => "tool call",
///         RequestModality::Embedding  => "embedding",
///         _                           => "unknown modality",
///     }
/// }
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub enum RequestModality {
    /// Plain-text chat completion — `messages` contains only text parts.
    Text,
    /// Multimodal chat completion — `messages` may contain image or audio
    /// [`ContentPart`](super::types::ContentPart)s.
    Multimodal,
    /// Chat completion that may invoke tool / function calls.
    ToolCall,
    /// Dense embedding generation — uses `embedding_input` rather than
    /// `messages`.
    Embedding,
}

// ─── Inference request ─────────────────────────────────────────────────────

/// Unified inference request envelope.
///
/// `InferenceRequest` is the single type that flows from callers into
/// [`InferenceProtocol`] implementations.  Backends convert it into their
/// native wire format (e.g. the OpenAI Chat Completions JSON body).
///
/// Use the static constructor helpers for the most common cases, then chain
/// the `with_*` builder methods to refine:
///
/// ```rust
/// use mofa_kernel::llm::irp::InferenceRequest;
/// use mofa_kernel::llm::types::{ChatMessage, Tool};
///
/// // Text
/// let req = InferenceRequest::text("gpt-4o", "Hello, world!");
///
/// // Multimodal (image + text)
/// let req = InferenceRequest::multimodal(
///     "gpt-4o",
///     vec![ChatMessage::user_with_image("Describe this image", "https://example.com/img.png")],
/// );
///
/// // Embedding
/// let req = InferenceRequest::embedding("text-embedding-3-small", "embed me");
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InferenceRequest {
    /// Target model identifier (e.g. `"gpt-4o"`, `"claude-3-opus"`).
    pub model: String,

    /// The primary modality of this request.
    pub modality: RequestModality,

    /// Full conversation history used for text / multimodal / tool-call
    /// requests.  Empty for embedding requests.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub messages: Vec<ChatMessage>,

    /// Tools made available to the model (tool-call modality only).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<Tool>>,

    /// Explicit tool-selection policy.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_choice: Option<ToolChoice>,

    /// Sampling temperature in `[0.0, 2.0]`.  `None` lets the backend use
    /// its own default.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,

    /// Maximum tokens the model may generate.  `None` means no explicit cap.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<u32>,

    /// Embedding-specific payload.  Non-empty only when
    /// `modality == RequestModality::Embedding`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub embedding_input: Option<EmbeddingInput>,

    /// Whether to request streaming token generation.  This is a *hint* —
    /// backends may ignore it if they do not support streaming.
    #[serde(default)]
    pub stream: bool,
}

impl InferenceRequest {
    // ── Constructors ─────────────────────────────────────────────────────

    /// Create a plain-text chat request with a single user turn.
    ///
    /// ```rust
    /// use mofa_kernel::llm::irp::{InferenceRequest, RequestModality};
    ///
    /// let req = InferenceRequest::text("gpt-4o", "Hello");
    /// assert_eq!(req.model, "gpt-4o");
    /// assert_eq!(req.modality, RequestModality::Text);
    /// assert_eq!(req.messages.len(), 1);
    /// ```
    pub fn text(model: impl Into<String>, user_message: impl Into<String>) -> Self {
        Self {
            model: model.into(),
            modality: RequestModality::Text,
            messages: vec![ChatMessage::user(user_message)],
            tools: None,
            tool_choice: None,
            temperature: None,
            max_tokens: None,
            embedding_input: None,
            stream: false,
        }
    }

    /// Create a multimodal chat request from a pre-built message list.
    ///
    /// Use [`ChatMessage::user_with_parts`] or [`ChatMessage::user_with_image`]
    /// to attach image or audio [`ContentPart`](super::types::ContentPart)s.
    ///
    /// ```rust
    /// use mofa_kernel::llm::irp::{InferenceRequest, RequestModality};
    /// use mofa_kernel::llm::types::ChatMessage;
    ///
    /// let messages = vec![
    ///     ChatMessage::user_with_image("What is in this image?", "https://example.com/cat.jpg"),
    /// ];
    /// let req = InferenceRequest::multimodal("gpt-4o", messages);
    /// assert_eq!(req.modality, RequestModality::Multimodal);
    /// ```
    pub fn multimodal(model: impl Into<String>, messages: Vec<ChatMessage>) -> Self {
        Self {
            model: model.into(),
            modality: RequestModality::Multimodal,
            messages,
            tools: None,
            tool_choice: None,
            temperature: None,
            max_tokens: None,
            embedding_input: None,
            stream: false,
        }
    }

    /// Create a tool-calling chat request.
    ///
    /// ```rust
    /// use mofa_kernel::llm::irp::{InferenceRequest, RequestModality};
    /// use mofa_kernel::llm::types::{ChatMessage, Tool};
    ///
    /// let tools = vec![
    ///     Tool::function("get_weather", "Get current weather", serde_json::json!({})),
    /// ];
    /// let req = InferenceRequest::tool_call("gpt-4o", vec![ChatMessage::user("What is the weather?")], tools);
    /// assert_eq!(req.modality, RequestModality::ToolCall);
    /// assert!(req.tools.is_some());
    /// ```
    pub fn tool_call(
        model: impl Into<String>,
        messages: Vec<ChatMessage>,
        tools: Vec<Tool>,
    ) -> Self {
        Self {
            model: model.into(),
            modality: RequestModality::ToolCall,
            messages,
            tools: Some(tools),
            tool_choice: None,
            temperature: None,
            max_tokens: None,
            embedding_input: None,
            stream: false,
        }
    }

    /// Create an embedding request for a single string.
    ///
    /// ```rust
    /// use mofa_kernel::llm::irp::{InferenceRequest, RequestModality};
    ///
    /// let req = InferenceRequest::embedding("text-embedding-3-small", "Hello world");
    /// assert_eq!(req.modality, RequestModality::Embedding);
    /// assert!(req.messages.is_empty());
    /// ```
    pub fn embedding(model: impl Into<String>, input: impl Into<String>) -> Self {
        Self {
            model: model.into(),
            modality: RequestModality::Embedding,
            messages: vec![],
            tools: None,
            tool_choice: None,
            temperature: None,
            max_tokens: None,
            embedding_input: Some(EmbeddingInput::Single(input.into())),
            stream: false,
        }
    }

    /// Create an embedding request for a batch of strings.
    ///
    /// ```rust
    /// use mofa_kernel::llm::irp::{InferenceRequest, RequestModality};
    ///
    /// let req = InferenceRequest::embedding_batch(
    ///     "text-embedding-3-small",
    ///     vec!["Hello".into(), "World".into()],
    /// );
    /// assert_eq!(req.modality, RequestModality::Embedding);
    /// ```
    pub fn embedding_batch(model: impl Into<String>, inputs: Vec<String>) -> Self {
        Self {
            model: model.into(),
            modality: RequestModality::Embedding,
            messages: vec![],
            tools: None,
            tool_choice: None,
            temperature: None,
            max_tokens: None,
            embedding_input: Some(EmbeddingInput::Multiple(inputs)),
            stream: false,
        }
    }

    // ── Builder helpers ─────────────────────────────────────────────────

    /// Set the sampling temperature.
    ///
    /// # Errors
    ///
    /// Returns `Err` if `temperature` is outside `[0.0, 2.0]`.
    ///
    /// ```rust
    /// use mofa_kernel::llm::irp::InferenceRequest;
    ///
    /// assert!(InferenceRequest::text("m", "q").with_temperature(0.7).is_ok());
    /// assert!(InferenceRequest::text("m", "q").with_temperature(3.0).is_err());
    /// assert!(InferenceRequest::text("m", "q").with_temperature(-0.1).is_err());
    /// ```
    pub fn with_temperature(mut self, temperature: f32) -> Result<Self, &'static str> {
        if !(0.0..=2.0).contains(&temperature) {
            return Err("temperature must be in [0.0, 2.0]");
        }
        self.temperature = Some(temperature);
        Ok(self)
    }

    /// Set the maximum number of output tokens.
    ///
    /// # Errors
    ///
    /// Returns `Err` if `max_tokens` is `0`.
    ///
    /// ```rust
    /// use mofa_kernel::llm::irp::InferenceRequest;
    ///
    /// assert!(InferenceRequest::text("m", "q").with_max_tokens(512).is_ok());
    /// assert!(InferenceRequest::text("m", "q").with_max_tokens(0).is_err());
    /// ```
    pub fn with_max_tokens(mut self, max_tokens: u32) -> Result<Self, &'static str> {
        if max_tokens == 0 {
            return Err("max_tokens must be > 0");
        }
        self.max_tokens = Some(max_tokens);
        Ok(self)
    }

    /// Append a [`Tool`] and automatically upgrade the modality to
    /// [`RequestModality::ToolCall`] if it was previously `Text`.
    ///
    /// ```rust
    /// use mofa_kernel::llm::irp::{InferenceRequest, RequestModality};
    /// use mofa_kernel::llm::types::Tool;
    ///
    /// let req = InferenceRequest::text("gpt-4o", "call a tool")
    ///     .with_tool(Tool::function("f", "desc", serde_json::json!({})));
    /// assert_eq!(req.modality, RequestModality::ToolCall);
    /// ```
    pub fn with_tool(mut self, tool: Tool) -> Self {
        self.tools.get_or_insert_with(Vec::new).push(tool);
        self.modality = RequestModality::ToolCall;
        self
    }

    /// Set the explicit tool-choice policy.
    pub fn with_tool_choice(mut self, choice: ToolChoice) -> Self {
        self.tool_choice = Some(choice);
        self
    }

    /// Append a message to the conversation.
    pub fn with_message(mut self, message: ChatMessage) -> Self {
        self.messages.push(message);
        self
    }

    /// Enable the streaming hint.
    pub fn with_stream(mut self) -> Self {
        self.stream = true;
        self
    }

    // ── Conversion helpers ──────────────────────────────────────────────

    /// Convert into a [`ChatCompletionRequest`] for chat / multimodal /
    /// tool-call modalities.
    ///
    /// Returns `None` for embedding requests.
    ///
    /// ```rust
    /// use mofa_kernel::llm::irp::InferenceRequest;
    ///
    /// let req = InferenceRequest::text("gpt-4o", "Hi");
    /// assert!(req.into_chat_request().is_some());
    ///
    /// let emb = InferenceRequest::embedding("m", "text");
    /// assert!(emb.into_chat_request().is_none());
    /// ```
    pub fn into_chat_request(self) -> Option<ChatCompletionRequest> {
        match self.modality {
            RequestModality::Embedding => None,
            _ => {
                let mut req = ChatCompletionRequest::new(self.model);
                req.messages = self.messages;
                req.temperature = self.temperature;
                req.max_tokens = self.max_tokens;
                req.tools = self.tools;
                req.tool_choice = self.tool_choice;
                req.stream = if self.stream { Some(true) } else { None };
                Some(req)
            }
        }
    }

    /// Convert into an [`EmbeddingRequest`] for the embedding modality.
    ///
    /// Returns `None` for non-embedding requests.
    ///
    /// ```rust
    /// use mofa_kernel::llm::irp::InferenceRequest;
    ///
    /// let req = InferenceRequest::embedding("m", "text");
    /// assert!(req.into_embedding_request().is_some());
    ///
    /// let chat = InferenceRequest::text("m", "hi");
    /// assert!(chat.into_embedding_request().is_none());
    /// ```
    pub fn into_embedding_request(self) -> Option<EmbeddingRequest> {
        match (self.modality, self.embedding_input) {
            (RequestModality::Embedding, Some(input)) => {
                Some(EmbeddingRequest { model: self.model, input })
            }
            _ => None,
        }
    }
}

// ─── Inference response ────────────────────────────────────────────────────

/// Unified synchronous inference response.
///
/// Wraps the variant that corresponds to the request modality.  The enum is
/// `#[non_exhaustive]` so that new response kinds can be added without
/// breaking downstream matches.
///
/// ```rust
/// use mofa_kernel::llm::irp::InferenceResponse;
/// use mofa_kernel::llm::types::{ChatCompletionResponse, Choice, ChatMessage, Role};
///
/// let chat_resp = ChatCompletionResponse { choices: vec![] };
/// let resp = InferenceResponse::Chat(chat_resp);
/// assert!(!resp.has_tool_calls());
/// assert!(resp.text_content().is_none());
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub enum InferenceResponse {
    /// Response to a text, multimodal, or tool-call request.
    Chat(ChatCompletionResponse),
    /// Response to an embedding request.
    Embedding(EmbeddingResponse),
}

impl InferenceResponse {
    /// Extract the text content of the first choice, if any.
    ///
    /// Returns `None` for embedding responses or when the model produced only
    /// tool calls and no text output.
    pub fn text_content(&self) -> Option<&str> {
        match self {
            InferenceResponse::Chat(r) => r.content(),
            InferenceResponse::Embedding(_) => None,
        }
    }

    /// Returns `true` when the response contains at least one tool call.
    pub fn has_tool_calls(&self) -> bool {
        match self {
            InferenceResponse::Chat(r) => r.has_tool_calls(),
            InferenceResponse::Embedding(_) => false,
        }
    }

    /// Unwrap as a [`ChatCompletionResponse`].
    ///
    /// # Panics
    ///
    /// Panics if the variant is not [`InferenceResponse::Chat`].  Prefer
    /// pattern matching in production code.
    pub fn unwrap_chat(self) -> ChatCompletionResponse {
        match self {
            InferenceResponse::Chat(r) => r,
            InferenceResponse::Embedding(_) => panic!(
                "called `InferenceResponse::unwrap_chat()` on an `Embedding` variant"
            ),
        }
    }

    /// Unwrap as an [`EmbeddingResponse`].
    ///
    /// # Panics
    ///
    /// Panics if the variant is not [`InferenceResponse::Embedding`].  Prefer
    /// pattern matching in production code.
    pub fn unwrap_embedding(self) -> EmbeddingResponse {
        match self {
            InferenceResponse::Embedding(r) => r,
            InferenceResponse::Chat(_) => panic!(
                "called `InferenceResponse::unwrap_embedding()` on a `Chat` variant"
            ),
        }
    }
}

// ─── Inference capabilities ────────────────────────────────────────────────

/// Capability advertisement returned by an inference backend.
///
/// Backends populate this struct inside their
/// [`InferenceProtocol::capabilities`] implementation.  Callers inspect it
/// to decide whether a given [`RequestModality`] is serviceable before
/// constructing a request.
///
/// All fields default to `false` / `None` so that newly written backends are
/// automatically conservative until they explicitly opt in.
///
/// ```rust
/// use mofa_kernel::llm::irp::{InferenceCapabilities, RequestModality};
///
/// let caps = InferenceCapabilities {
///     streaming: true,
///     tool_calling: true,
///     multimodal: false,
///     embedding: true,
///     ..Default::default()
/// };
///
/// assert!(caps.supports_modality(&RequestModality::Text));
/// assert!(caps.supports_modality(&RequestModality::ToolCall));
/// assert!(!caps.supports_modality(&RequestModality::Multimodal));
/// assert!(caps.supports_modality(&RequestModality::Embedding));
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct InferenceCapabilities {
    /// Supports streaming token generation.
    pub streaming: bool,
    /// Supports tool / function calling.
    pub tool_calling: bool,
    /// Supports multimodal inputs (images, audio).
    pub multimodal: bool,
    /// Supports dense embedding generation.
    pub embedding: bool,
    /// Supports JSON-mode structured output.
    pub json_mode: bool,
    /// Supports JSON-schema constrained output.
    pub json_schema: bool,
    /// Maximum context window in tokens, if known.
    pub max_context_tokens: Option<u32>,
    /// Maximum output tokens per request, if known.
    pub max_output_tokens: Option<u32>,
}

impl InferenceCapabilities {
    /// Returns `true` when this backend can service the given modality.
    ///
    /// `RequestModality::Text` always returns `true` because every functional
    /// backend must support plain-text chat.  Unknown / future modalities
    /// (non-exhaustive catch-all) return `false` conservatively.
    ///
    /// ```rust
    /// use mofa_kernel::llm::irp::{InferenceCapabilities, RequestModality};
    ///
    /// let caps = InferenceCapabilities::default();
    ///
    /// // Text is always supported
    /// assert!(caps.supports_modality(&RequestModality::Text));
    /// // Everything else defaults to false
    /// assert!(!caps.supports_modality(&RequestModality::Multimodal));
    /// assert!(!caps.supports_modality(&RequestModality::ToolCall));
    /// assert!(!caps.supports_modality(&RequestModality::Embedding));
    /// ```
    pub fn supports_modality(&self, modality: &RequestModality) -> bool {
        match modality {
            RequestModality::Text => true,
            RequestModality::Multimodal => self.multimodal,
            RequestModality::ToolCall => self.tool_calling,
            RequestModality::Embedding => self.embedding,
            // Future non-exhaustive variants: conservative default.
            _ => false,
        }
    }
}

// ─── Inference protocol ────────────────────────────────────────────────────

/// Kernel-level trait that formalises the Inference Request Protocol.
///
/// `InferenceProtocol` provides:
///
/// * [`capabilities`](InferenceProtocol::capabilities) — synchronous
///   advertisement of what a backend can do.
/// * [`infer`](InferenceProtocol::infer) — single async dispatch point with a
///   default routing implementation based on [`RequestModality`].
/// * Specialised methods ([`infer_chat`], [`infer_multimodal`],
///   [`infer_embedding`]) that backends override à la carte.
///
/// ## Blanket implementation
///
/// Any type that already implements
/// [`LLMProvider`](super::provider::LLMProvider) gets a free blanket
/// `InferenceProtocol` implementation from `provider.rs`.  No manual wiring
/// is required.
///
/// ## Implementing from scratch
///
/// ```rust,ignore
/// use async_trait::async_trait;
/// use mofa_kernel::llm::irp::{
///     InferenceCapabilities, InferenceProtocol, InferenceResponse,
/// };
/// use mofa_kernel::llm::types::{ChatCompletionRequest, ChatCompletionResponse};
/// use mofa_kernel::agent::AgentResult;
///
/// struct MyBackend;
///
/// #[async_trait]
/// impl InferenceProtocol for MyBackend {
///     fn capabilities(&self) -> InferenceCapabilities {
///         InferenceCapabilities { streaming: true, ..Default::default() }
///     }
///
///     async fn infer_chat(
///         &self,
///         _request: ChatCompletionRequest,
///     ) -> AgentResult<InferenceResponse> {
///         // … call the backend …
///         todo!()
///     }
/// }
/// ```
///
/// [`infer_chat`]: InferenceProtocol::infer_chat
/// [`infer_multimodal`]: InferenceProtocol::infer_multimodal
/// [`infer_embedding`]: InferenceProtocol::infer_embedding
#[async_trait]
pub trait InferenceProtocol: Send + Sync {
    /// Return the advertised capabilities of this backend.
    fn capabilities(&self) -> InferenceCapabilities;

    /// Dispatch a unified [`InferenceRequest`] to the appropriate backend
    /// method.
    ///
    /// The default implementation routes on [`InferenceRequest::modality`] and
    /// delegates to [`infer_chat`](InferenceProtocol::infer_chat),
    /// [`infer_multimodal`](InferenceProtocol::infer_multimodal), or
    /// [`infer_embedding`](InferenceProtocol::infer_embedding).
    ///
    /// Override this method only when you need full control over the dispatch
    /// logic.
    async fn infer(&self, request: InferenceRequest) -> AgentResult<InferenceResponse> {
        match request.modality {
            RequestModality::Text | RequestModality::ToolCall => {
                let chat_req = request
                    .into_chat_request()
                    .expect("Text/ToolCall modality always produces a ChatCompletionRequest");
                self.infer_chat(chat_req).await
            }
            RequestModality::Multimodal => {
                let chat_req = request
                    .into_chat_request()
                    .expect("Multimodal modality always produces a ChatCompletionRequest");
                self.infer_multimodal(chat_req).await
            }
            RequestModality::Embedding => {
                let emb_req = request
                    .into_embedding_request()
                    .expect("Embedding modality always produces an EmbeddingRequest");
                self.infer_embedding(emb_req).await
            }
            // Future non-exhaustive variants: return a clear error.
            _ => Err(crate::agent::AgentError::Other(
                "Unsupported request modality".into(),
            )),
        }
    }

    /// Perform a text or tool-call chat completion.
    ///
    /// This is the only *required* method — every backend must handle basic
    /// chat requests.
    async fn infer_chat(
        &self,
        request: ChatCompletionRequest,
    ) -> AgentResult<InferenceResponse>;

    /// Perform a multimodal chat completion (text + images / audio).
    ///
    /// **Default**: delegates to [`infer_chat`](InferenceProtocol::infer_chat).
    /// Override when your backend requires special pre-processing for
    /// non-text content parts (e.g. asset upload before the request).
    async fn infer_multimodal(
        &self,
        request: ChatCompletionRequest,
    ) -> AgentResult<InferenceResponse> {
        self.infer_chat(request).await
    }

    /// Perform an embedding request.
    ///
    /// **Default**: returns `Err` — backends must explicitly opt in by
    /// overriding this method.
    async fn infer_embedding(
        &self,
        _request: EmbeddingRequest,
    ) -> AgentResult<InferenceResponse> {
        Err(crate::agent::AgentError::Other(
            "This backend does not support embeddings".into(),
        ))
    }
}

// ─── Unit tests ────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::llm::types::{
        ChatCompletionResponse, Choice, EmbeddingData, EmbeddingResponse, EmbeddingUsage,
        FunctionDefinition, Tool,
    };

    // ── InferenceRequest constructors ─────────────────────────────────────

    #[test]
    fn text_request_has_correct_defaults() {
        let req = InferenceRequest::text("gpt-4o", "Hello");
        assert_eq!(req.model, "gpt-4o");
        assert_eq!(req.modality, RequestModality::Text);
        assert_eq!(req.messages.len(), 1);
        assert!(req.tools.is_none());
        assert!(req.embedding_input.is_none());
        assert!(!req.stream);
        assert!(req.temperature.is_none());
        assert!(req.max_tokens.is_none());
    }

    #[test]
    fn multimodal_request_preserves_messages() {
        let messages = vec![
            ChatMessage::user_with_image("Describe", "https://example.com/cat.jpg"),
        ];
        let req = InferenceRequest::multimodal("gpt-4o", messages.clone());
        assert_eq!(req.modality, RequestModality::Multimodal);
        assert_eq!(req.messages.len(), 1);
    }

    #[test]
    fn tool_call_request_attaches_tools() {
        let tool = Tool::function("get_weather", "Get weather", serde_json::json!({}));
        let req = InferenceRequest::tool_call(
            "gpt-4o",
            vec![ChatMessage::user("What is the weather?")],
            vec![tool],
        );
        assert_eq!(req.modality, RequestModality::ToolCall);
        assert_eq!(req.tools.as_ref().unwrap().len(), 1);
        assert!(!req.messages.is_empty());
    }

    #[test]
    fn embedding_single_request_is_correct() {
        let req = InferenceRequest::embedding("text-embedding-3-small", "Hello world");
        assert_eq!(req.modality, RequestModality::Embedding);
        assert!(req.messages.is_empty());
        assert!(req.embedding_input.is_some());
        matches!(req.embedding_input, Some(EmbeddingInput::Single(_)));
    }

    #[test]
    fn embedding_batch_request_is_correct() {
        let req = InferenceRequest::embedding_batch(
            "text-embedding-3-small",
            vec!["a".into(), "b".into()],
        );
        assert_eq!(req.modality, RequestModality::Embedding);
        matches!(req.embedding_input, Some(EmbeddingInput::Multiple(_)));
    }

    // ── Builder validation ────────────────────────────────────────────────

    #[test]
    fn with_temperature_accepts_valid_range() {
        let r = InferenceRequest::text("m", "q").with_temperature(0.0);
        assert!(r.is_ok());
        let r = InferenceRequest::text("m", "q").with_temperature(2.0);
        assert!(r.is_ok());
        let r = InferenceRequest::text("m", "q").with_temperature(1.0);
        assert_eq!(r.unwrap().temperature, Some(1.0));
    }

    #[test]
    fn with_temperature_rejects_out_of_range() {
        assert!(InferenceRequest::text("m", "q").with_temperature(-0.1).is_err());
        assert!(InferenceRequest::text("m", "q").with_temperature(2.01).is_err());
    }

    #[test]
    fn with_max_tokens_accepts_positive() {
        let r = InferenceRequest::text("m", "q").with_max_tokens(1);
        assert!(r.is_ok());
        assert_eq!(r.unwrap().max_tokens, Some(1));
    }

    #[test]
    fn with_max_tokens_rejects_zero() {
        assert!(InferenceRequest::text("m", "q").with_max_tokens(0).is_err());
    }

    #[test]
    fn with_tool_upgrades_modality_to_tool_call() {
        let tool = Tool::function("f", "d", serde_json::json!({}));
        let req = InferenceRequest::text("m", "q").with_tool(tool);
        assert_eq!(req.modality, RequestModality::ToolCall);
        assert_eq!(req.tools.unwrap().len(), 1);
    }

    #[test]
    fn with_stream_sets_flag() {
        let req = InferenceRequest::text("m", "q").with_stream();
        assert!(req.stream);
    }

    #[test]
    fn with_message_appends_to_conversation() {
        let req = InferenceRequest::text("m", "first")
            .with_message(ChatMessage::assistant("response"))
            .with_message(ChatMessage::user("follow-up"));
        assert_eq!(req.messages.len(), 3);
    }

    // ── Conversion helpers ────────────────────────────────────────────────

    #[test]
    fn text_request_converts_to_chat() {
        let req = InferenceRequest::text("gpt-4o", "Hello");
        let chat = req.into_chat_request();
        assert!(chat.is_some());
        assert_eq!(chat.unwrap().model, "gpt-4o");
    }

    #[test]
    fn multimodal_request_converts_to_chat() {
        let req = InferenceRequest::multimodal(
            "gpt-4o",
            vec![ChatMessage::user("test")],
        );
        assert!(req.into_chat_request().is_some());
    }

    #[test]
    fn tool_call_request_converts_to_chat() {
        let tool = Tool::function("f", "d", serde_json::json!({}));
        let req = InferenceRequest::tool_call(
            "m",
            vec![ChatMessage::user("q")],
            vec![tool],
        );
        let chat = req.into_chat_request().unwrap();
        assert!(chat.tools.is_some());
    }

    #[test]
    fn embedding_request_does_not_convert_to_chat() {
        let req = InferenceRequest::embedding("m", "text");
        assert!(req.into_chat_request().is_none());
    }

    #[test]
    fn embedding_request_converts_to_embedding() {
        let req = InferenceRequest::embedding("m", "text");
        let emb = req.into_embedding_request();
        assert!(emb.is_some());
        assert_eq!(emb.unwrap().model, "m");
    }

    #[test]
    fn text_request_does_not_convert_to_embedding() {
        let req = InferenceRequest::text("m", "hello");
        assert!(req.into_embedding_request().is_none());
    }

    #[test]
    fn chat_request_stream_flag_propagates() {
        let chat = InferenceRequest::text("m", "q")
            .with_stream()
            .into_chat_request()
            .unwrap();
        assert_eq!(chat.stream, Some(true));
    }

    #[test]
    fn chat_request_no_stream_flag_is_none() {
        let chat = InferenceRequest::text("m", "q")
            .into_chat_request()
            .unwrap();
        assert!(chat.stream.is_none());
    }

    // ── InferenceCapabilities ─────────────────────────────────────────────

    #[test]
    fn default_capabilities_are_conservative() {
        let caps = InferenceCapabilities::default();
        assert!(!caps.streaming);
        assert!(!caps.tool_calling);
        assert!(!caps.multimodal);
        assert!(!caps.embedding);
        assert!(!caps.json_mode);
        assert!(!caps.json_schema);
        assert!(caps.max_context_tokens.is_none());
        assert!(caps.max_output_tokens.is_none());
    }

    #[test]
    fn supports_modality_text_always_true() {
        let caps = InferenceCapabilities::default();
        assert!(caps.supports_modality(&RequestModality::Text));
    }

    #[test]
    fn supports_modality_respects_flags() {
        let caps = InferenceCapabilities {
            tool_calling: true,
            multimodal: true,
            embedding: true,
            ..Default::default()
        };
        assert!(caps.supports_modality(&RequestModality::ToolCall));
        assert!(caps.supports_modality(&RequestModality::Multimodal));
        assert!(caps.supports_modality(&RequestModality::Embedding));
    }

    #[test]
    fn supports_modality_false_when_disabled() {
        let caps = InferenceCapabilities::default();
        assert!(!caps.supports_modality(&RequestModality::ToolCall));
        assert!(!caps.supports_modality(&RequestModality::Multimodal));
        assert!(!caps.supports_modality(&RequestModality::Embedding));
    }

    #[test]
    fn capabilities_equality() {
        let a = InferenceCapabilities { streaming: true, ..Default::default() };
        let b = InferenceCapabilities { streaming: true, ..Default::default() };
        let c = InferenceCapabilities { streaming: false, ..Default::default() };
        assert_eq!(a, b);
        assert_ne!(a, c);
    }

    // ── InferenceResponse ─────────────────────────────────────────────────

    fn make_chat_response(content: &str) -> ChatCompletionResponse {
        use crate::llm::types::{MessageContent, Role};
        ChatCompletionResponse {
            choices: vec![Choice {
                index: 0,
                message: ChatMessage {
                    role: Role::Assistant,
                    content: Some(MessageContent::Text(content.to_string())),
                    name: None,
                    tool_calls: None,
                    tool_call_id: None,
                },
                finish_reason: None,
                logprobs: None,
            }],
        }
    }

    fn make_embedding_response() -> EmbeddingResponse {
        EmbeddingResponse {
            data: vec![EmbeddingData {
                object: "embedding".into(),
                index: 0,
                embedding: vec![0.1, 0.2, 0.3],
            }],
            usage: Some(EmbeddingUsage {
                prompt_tokens: 3,
                total_tokens: 3,
            }),
        }
    }

    #[test]
    fn chat_response_text_content() {
        let resp = InferenceResponse::Chat(make_chat_response("hello"));
        assert_eq!(resp.text_content(), Some("hello"));
    }

    #[test]
    fn embedding_response_text_content_is_none() {
        let resp = InferenceResponse::Embedding(make_embedding_response());
        assert!(resp.text_content().is_none());
    }

    #[test]
    fn chat_response_no_tool_calls() {
        let resp = InferenceResponse::Chat(make_chat_response("hi"));
        assert!(!resp.has_tool_calls());
    }

    #[test]
    fn embedding_response_has_no_tool_calls() {
        let resp = InferenceResponse::Embedding(make_embedding_response());
        assert!(!resp.has_tool_calls());
    }

    #[test]
    fn unwrap_chat_succeeds() {
        let chat = make_chat_response("result");
        let resp = InferenceResponse::Chat(chat.clone());
        let unwrapped = resp.unwrap_chat();
        assert_eq!(unwrapped.content(), Some("result"));
    }

    #[test]
    fn unwrap_embedding_succeeds() {
        let emb = make_embedding_response();
        let resp = InferenceResponse::Embedding(emb);
        let unwrapped = resp.unwrap_embedding();
        assert_eq!(unwrapped.data.len(), 1);
    }

    #[test]
    #[should_panic(expected = "unwrap_chat")]
    fn unwrap_chat_panics_on_embedding() {
        let resp = InferenceResponse::Embedding(make_embedding_response());
        let _ = resp.unwrap_chat();
    }

    #[test]
    #[should_panic(expected = "unwrap_embedding")]
    fn unwrap_embedding_panics_on_chat() {
        let resp = InferenceResponse::Chat(make_chat_response("hi"));
        let _ = resp.unwrap_embedding();
    }

    // ── Serialization round-trip ──────────────────────────────────────────

    #[test]
    fn inference_request_serializes_and_deserializes() {
        let req = InferenceRequest::text("gpt-4o", "Hello")
            .with_temperature(0.5)
            .unwrap()
            .with_max_tokens(256)
            .unwrap()
            .with_stream();

        let json = serde_json::to_string(&req).expect("serialize");
        let de: InferenceRequest = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(de.model, "gpt-4o");
        assert_eq!(de.temperature, Some(0.5));
        assert_eq!(de.max_tokens, Some(256));
        assert!(de.stream);
    }

    #[test]
    fn inference_capabilities_serializes_and_deserializes() {
        let caps = InferenceCapabilities {
            streaming: true,
            embedding: true,
            max_context_tokens: Some(128_000),
            ..Default::default()
        };
        let json = serde_json::to_string(&caps).expect("serialize");
        let de: InferenceCapabilities = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(de, caps);
    }
}
