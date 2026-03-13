//! LLM Provider Fallback Chain
//!
//! Wraps multiple [`LLMProvider`]s in priority order. When a provider fails
//! with a triggering error (rate-limit, quota, network, timeout, auth), the
//! chain automatically tries the next provider. The chain itself implements
//! [`LLMProvider`], so it is transparent to the rest of the system.
//!
//! # Example
//!
//! ```rust,ignore
//! use mofa_foundation::llm::{FallbackChain, FallbackCondition};
//!
//! let chain = FallbackChain::builder()
//!     .add(openai_provider)                              // try first
//!     .add_with_trigger(anthropic_provider,              // try if openai is rate-limited
//!         FallbackTrigger::on_conditions(vec![
//!             FallbackCondition::RateLimited,
//!             FallbackCondition::QuotaExceeded,
//!         ]))
//!     .add_last(ollama_provider)                         // last resort, always try
//!     .build();
//!
//! // Use exactly like any other LLMProvider
//! let client = LLMClient::new(Arc::new(chain));
//! ```

use super::provider::LLMProvider;
use super::types::*;
use async_trait::async_trait;
use std::sync::Arc;
use tracing::{info, warn};

// Fallback Condition

/// Conditions under which the chain moves to the next provider.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum FallbackCondition {
    /// Provider returned HTTP 429 / rate-limit response.
    RateLimited,
    /// Provider returned quota-exceeded / billing error.
    QuotaExceeded,
    /// TCP/TLS/DNS failure reaching the provider.
    NetworkError,
    /// Request took too long and was aborted.
    Timeout,
    /// API key rejected / credentials invalid.
    AuthError,
    /// Provider does not support the requested model or feature.
    ProviderUnavailable,
    /// Prompt is too long for the provider's context window.
    ContextLengthExceeded,
    /// Requested model does not exist on this provider.
    ModelNotFound,
}

impl FallbackCondition {
    /// Returns `true` when `error` matches this condition.
    pub fn matches(&self, error: &LLMError) -> bool {
        match (self, error) {
            (Self::RateLimited, LLMError::RateLimited(_)) => true,
            (Self::QuotaExceeded, LLMError::QuotaExceeded(_)) => true,
            (Self::NetworkError, LLMError::NetworkError(_)) => true,
            (Self::Timeout, LLMError::Timeout(_)) => true,
            (Self::AuthError, LLMError::AuthError(_)) => true,
            (Self::ProviderUnavailable, LLMError::ProviderNotSupported(_)) => true,
            (Self::ContextLengthExceeded, LLMError::ContextLengthExceeded(_)) => true,
            (Self::ModelNotFound, LLMError::ModelNotFound(_)) => true,
            _ => false,
        }
    }

    /// Default set of conditions used when none are specified.
    ///
    /// Covers transient, provider-side failures that make sense to retry
    /// against a different backend:
    /// - `RateLimited`
    /// - `QuotaExceeded`
    /// - `NetworkError`
    /// - `Timeout`
    /// - `AuthError`
    pub fn defaults() -> Vec<Self> {
        vec![
            Self::RateLimited,
            Self::QuotaExceeded,
            Self::NetworkError,
            Self::Timeout,
            Self::AuthError,
        ]
    }
}

// Fallback Trigger

/// Controls when a provider entry triggers a fallback to the next provider.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub enum FallbackTrigger {
    /// Fall back only when the error matches one of the listed conditions.
    OnConditions(Vec<FallbackCondition>),
    /// Fall back on any error.
    OnAnyError,
    /// Never fall back — this entry is always the terminal provider.
    Never,
}

impl FallbackTrigger {
    /// Convenience constructor for condition-based trigger.
    pub fn on_conditions(conditions: Vec<FallbackCondition>) -> Self {
        Self::OnConditions(conditions)
    }

    /// Returns the default trigger using [`FallbackCondition::defaults`].
    pub fn default_conditions() -> Self {
        Self::OnConditions(FallbackCondition::defaults())
    }

    /// Returns `true` if `error` should cause the chain to move to the next
    /// provider.
    pub fn should_fallback(&self, error: &LLMError) -> bool {
        match self {
            Self::OnConditions(conditions) => conditions.iter().any(|c| c.matches(error)),
            Self::OnAnyError => true,
            Self::Never => false,
        }
    }
}

impl Default for FallbackTrigger {
    fn default() -> Self {
        Self::default_conditions()
    }
}

// FallbackEntry

/// A single slot in the chain: a provider paired with its fallback trigger.
struct FallbackEntry {
    provider: Arc<dyn LLMProvider>,
    trigger: FallbackTrigger,
}

// FallbackChain

/// An [`LLMProvider`] that delegates to a prioritised list of providers and
/// automatically falls back when one fails.
///
/// Build with [`FallbackChain::builder`].
pub struct FallbackChain {
    name: String,
    providers: Vec<FallbackEntry>,
}

impl FallbackChain {
    /// Start building a new chain.
    pub fn builder() -> FallbackChainBuilder {
        FallbackChainBuilder::new()
    }

    /// Number of provider slots in the chain.
    pub fn len(&self) -> usize {
        self.providers.len()
    }

    /// `true` if the chain has no providers.
    pub fn is_empty(&self) -> bool {
        self.providers.is_empty()
    }

    /// Try providers in order for a non-streaming chat request.
    async fn try_chat(
        &self,
        request: ChatCompletionRequest,
    ) -> LLMResult<ChatCompletionResponse> {
        let mut last_error: Option<LLMError> = None;

        for (index, entry) in self.providers.iter().enumerate() {
            let provider_name = entry.provider.name();
            match entry.provider.chat(request.clone()).await {
                Ok(response) => {
                    if index > 0 {
                        info!(
                            chain = %self.name,
                            provider = %provider_name,
                            slot = index,
                            "FallbackChain: succeeded after fallback"
                        );
                    }
                    return Ok(response);
                }
                Err(err) => {
                    let is_last = index + 1 >= self.providers.len();
                    if !is_last && entry.trigger.should_fallback(&err) {
                        warn!(
                            chain = %self.name,
                            provider = %provider_name,
                            slot = index,
                            error = %err,
                            "FallbackChain: provider failed, trying next"
                        );
                        last_error = Some(err);
                    } else {
                        // Non-fallback error or last provider — propagate.
                        return Err(err);
                    }
                }
            }
        }

        Err(last_error
            .unwrap_or_else(|| LLMError::Other("FallbackChain has no providers".into())))
    }
}

#[async_trait]
impl LLMProvider for FallbackChain {
    fn name(&self) -> &str {
        &self.name
    }

    fn default_model(&self) -> &str {
        self.providers
            .first()
            .map(|e| e.provider.default_model())
            .unwrap_or("")
    }

    fn supports_streaming(&self) -> bool {
        self.providers
            .iter()
            .any(|e| e.provider.supports_streaming())
    }

    fn supports_tools(&self) -> bool {
        self.providers.iter().any(|e| e.provider.supports_tools())
    }

    fn supports_vision(&self) -> bool {
        self.providers.iter().any(|e| e.provider.supports_vision())
    }

    fn supports_embedding(&self) -> bool {
        self.providers
            .iter()
            .any(|e| e.provider.supports_embedding())
    }

    async fn chat(&self, request: ChatCompletionRequest) -> LLMResult<ChatCompletionResponse> {
        self.try_chat(request).await
    }

    async fn chat_stream(&self, request: ChatCompletionRequest) -> LLMResult<super::provider::ChatStream> {
        let mut last_error: Option<LLMError> = None;

        for (index, entry) in self.providers.iter().enumerate() {
            if !entry.provider.supports_streaming() {
                continue;
            }
            let provider_name = entry.provider.name();
            match entry.provider.chat_stream(request.clone()).await {
                Ok(stream) => {
                    if index > 0 {
                        info!(
                            chain = %self.name,
                            provider = %provider_name,
                            slot = index,
                            "FallbackChain: streaming succeeded after fallback"
                        );
                    }
                    return Ok(stream);
                }
                Err(err) => {
                    let is_last = index + 1 >= self.providers.len();
                    if !is_last && entry.trigger.should_fallback(&err) {
                        warn!(
                            chain = %self.name,
                            provider = %provider_name,
                            slot = index,
                            error = %err,
                            "FallbackChain: streaming provider failed, trying next"
                        );
                        last_error = Some(err);
                    } else {
                        return Err(err);
                    }
                }
            }
        }

        Err(last_error.unwrap_or_else(|| {
            LLMError::ProviderNotSupported(format!(
                "FallbackChain({}): no provider supports streaming",
                self.name
            ))
        }))
    }

    async fn embedding(&self, request: EmbeddingRequest) -> LLMResult<EmbeddingResponse> {
        let mut last_error: Option<LLMError> = None;

        for (index, entry) in self.providers.iter().enumerate() {
            if !entry.provider.supports_embedding() {
                continue;
            }
            let provider_name = entry.provider.name();
            match entry.provider.embedding(request.clone()).await {
                Ok(response) => {
                    if index > 0 {
                        info!(
                            chain = %self.name,
                            provider = %provider_name,
                            slot = index,
                            "FallbackChain: embedding succeeded after fallback"
                        );
                    }
                    return Ok(response);
                }
                Err(err) => {
                    let is_last = index + 1 >= self.providers.len();
                    if !is_last && entry.trigger.should_fallback(&err) {
                        warn!(
                            chain = %self.name,
                            provider = %provider_name,
                            slot = index,
                            error = %err,
                            "FallbackChain: embedding provider failed, trying next"
                        );
                        last_error = Some(err);
                    } else {
                        return Err(err);
                    }
                }
            }
        }

        Err(last_error.unwrap_or_else(|| {
            LLMError::ProviderNotSupported(format!(
                "FallbackChain({}): no provider supports embedding",
                self.name
            ))
        }))
    }

    async fn health_check(&self) -> LLMResult<bool> {
        // Healthy if at least one provider is healthy.
        for entry in &self.providers {
            if entry.provider.health_check().await.unwrap_or(false) {
                return Ok(true);
            }
        }
        Ok(false)
    }
}

// ============================================================================
// Builder
// ============================================================================

/// Builder for [`FallbackChain`].
pub struct FallbackChainBuilder {
    name: String,
    providers: Vec<FallbackEntry>,
}

impl FallbackChainBuilder {
    fn new() -> Self {
        Self {
            name: "fallback-chain".to_string(),
            providers: Vec::new(),
        }
    }

    /// Set the name reported by [`LLMProvider::name`].
    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.name = name.into();
        self
    }

    /// Add a provider using the default fallback conditions.
    ///
    /// Triggers fallback on: `RateLimited`, `QuotaExceeded`, `NetworkError`,
    /// `Timeout`, `AuthError`.
    pub fn add(self, provider: impl LLMProvider + 'static) -> Self {
        self.add_arc(Arc::new(provider), FallbackTrigger::default_conditions())
    }

    /// Add an `Arc<dyn LLMProvider>` using the default fallback conditions.
    pub fn add_shared(self, provider: Arc<dyn LLMProvider>) -> Self {
        self.add_arc(provider, FallbackTrigger::default_conditions())
    }

    /// Add a provider with a custom [`FallbackTrigger`].
    pub fn add_with_trigger(
        self,
        provider: impl LLMProvider + 'static,
        trigger: FallbackTrigger,
    ) -> Self {
        self.add_arc(Arc::new(provider), trigger)
    }

    /// Add the terminal (last-resort) provider — never falls back from here.
    pub fn add_last(self, provider: impl LLMProvider + 'static) -> Self {
        self.add_arc(Arc::new(provider), FallbackTrigger::Never)
    }

    /// Add a shared terminal provider — never falls back from here.
    pub fn add_last_shared(self, provider: Arc<dyn LLMProvider>) -> Self {
        self.add_arc(provider, FallbackTrigger::Never)
    }

    fn add_arc(mut self, provider: Arc<dyn LLMProvider>, trigger: FallbackTrigger) -> Self {
        self.providers.push(FallbackEntry { provider, trigger });
        self
    }

    /// Build the [`FallbackChain`].
    ///
    /// # Panics
    ///
    /// Panics if no providers were added.
    pub fn build(self) -> FallbackChain {
        assert!(
            !self.providers.is_empty(),
            "FallbackChainBuilder: at least one provider is required"
        );
        FallbackChain {
            name: self.name,
            providers: self.providers,
        }
    }
}

// Tests

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    /// A controllable mock provider that returns pre-set responses in sequence.
    struct MockProvider {
        name: String,
        responses: Vec<LLMResult<ChatCompletionResponse>>,
        call_count: AtomicUsize,
    }

    impl MockProvider {
        fn new(name: &str, responses: Vec<LLMResult<ChatCompletionResponse>>) -> Self {
            Self {
                name: name.to_string(),
                responses,
                call_count: AtomicUsize::new(0),
            }
        }

        fn calls(&self) -> usize {
            self.call_count.load(Ordering::SeqCst)
        }
    }

    #[async_trait]
    impl LLMProvider for MockProvider {
        fn name(&self) -> &str {
            &self.name
        }

        async fn chat(
            &self,
            _request: ChatCompletionRequest,
        ) -> LLMResult<ChatCompletionResponse> {
            let idx = self.call_count.fetch_add(1, Ordering::SeqCst);
            self.responses
                .get(idx)
                .cloned()
                .unwrap_or_else(|| Err(LLMError::Other("no more responses".into())))
        }
    }

    fn ok_response(text: &str) -> LLMResult<ChatCompletionResponse> {
        Ok(ChatCompletionResponse {
            id: "id".into(),
            object: "chat.completion".into(),
            created: 0,
            model: "model".into(),
            choices: vec![Choice {
                index: 0,
                message: ChatMessage::assistant(text),
                finish_reason: Some(FinishReason::Stop),
                logprobs: None,
            }],
            usage: None,
            system_fingerprint: None,
        })
    }

    fn request() -> ChatCompletionRequest {
        ChatCompletionRequest::new("test-model")
    }

    // FallbackCondition::matches

    #[test]
    fn condition_matches_correct_errors() {
        assert!(FallbackCondition::RateLimited.matches(&LLMError::RateLimited("x".into())));
        assert!(FallbackCondition::QuotaExceeded.matches(&LLMError::QuotaExceeded("x".into())));
        assert!(FallbackCondition::NetworkError.matches(&LLMError::NetworkError("x".into())));
        assert!(FallbackCondition::Timeout.matches(&LLMError::Timeout("x".into())));
        assert!(FallbackCondition::AuthError.matches(&LLMError::AuthError("x".into())));
        assert!(FallbackCondition::ModelNotFound.matches(&LLMError::ModelNotFound("x".into())));
        assert!(FallbackCondition::ContextLengthExceeded
            .matches(&LLMError::ContextLengthExceeded("x".into())));
        assert!(FallbackCondition::ProviderUnavailable
            .matches(&LLMError::ProviderNotSupported("x".into())));
    }

    #[test]
    fn condition_does_not_match_unrelated_error() {
        assert!(!FallbackCondition::RateLimited.matches(&LLMError::AuthError("x".into())));
        assert!(!FallbackCondition::NetworkError.matches(&LLMError::RateLimited("x".into())));
    }

    // FallbackTrigger::should_fallback

    #[test]
    fn trigger_on_any_error_always_falls_back() {
        let t = FallbackTrigger::OnAnyError;
        assert!(t.should_fallback(&LLMError::RateLimited("x".into())));
        assert!(t.should_fallback(&LLMError::Other("x".into())));
        assert!(t.should_fallback(&LLMError::SerializationError("x".into())));
    }

    #[test]
    fn trigger_never_never_falls_back() {
        let t = FallbackTrigger::Never;
        assert!(!t.should_fallback(&LLMError::RateLimited("x".into())));
        assert!(!t.should_fallback(&LLMError::NetworkError("x".into())));
    }

    #[test]
    fn trigger_on_conditions_respects_list() {
        let t = FallbackTrigger::on_conditions(vec![FallbackCondition::RateLimited]);
        assert!(t.should_fallback(&LLMError::RateLimited("x".into())));
        assert!(!t.should_fallback(&LLMError::NetworkError("x".into())));
    }

    // FallbackChain basic scenarios

    #[tokio::test]
    async fn first_provider_succeeds_no_fallback() {
        let p1 = MockProvider::new("p1", vec![ok_response("hello")]);
        let p2 = MockProvider::new("p2", vec![ok_response("world")]);

        let p1 = Arc::new(p1);
        let p2 = Arc::new(p2);
        let p1_ref = p1.clone();
        let p2_ref = p2.clone();

        let chain = FallbackChain::builder()
            .add_shared(p1)
            .add_last_shared(p2)
            .build();

        let result = chain.chat(request()).await.unwrap();
        assert_eq!(result.content().unwrap(), "hello");
        assert_eq!(p1_ref.calls(), 1);
        assert_eq!(p2_ref.calls(), 0);
    }

    #[tokio::test]
    async fn falls_back_on_rate_limit() {
        let p1 = MockProvider::new("p1", vec![Err(LLMError::RateLimited("429".into()))]);
        let p2 = MockProvider::new("p2", vec![ok_response("from-p2")]);

        let chain = FallbackChain::builder()
            .add(p1)
            .add_last(p2)
            .build();

        let result = chain.chat(request()).await.unwrap();
        assert_eq!(result.content().unwrap(), "from-p2");
    }

    #[tokio::test]
    async fn falls_back_through_all_providers() {
        let p1 = MockProvider::new("p1", vec![Err(LLMError::RateLimited("rl".into()))]);
        let p2 = MockProvider::new("p2", vec![Err(LLMError::QuotaExceeded("quota".into()))]);
        let p3 = MockProvider::new("p3", vec![ok_response("p3-ok")]);

        let chain = FallbackChain::builder()
            .add(p1)
            .add(p2)
            .add_last(p3)
            .build();

        let result = chain.chat(request()).await.unwrap();
        assert_eq!(result.content().unwrap(), "p3-ok");
    }

    #[tokio::test]
    async fn propagates_non_fallback_error() {
        // SerializationError is not in the default trigger conditions.
        let p1 = MockProvider::new(
            "p1",
            vec![Err(LLMError::SerializationError("bad json".into()))],
        );
        let p2 = MockProvider::new("p2", vec![ok_response("should not reach")]);

        let chain = FallbackChain::builder()
            .add(p1)
            .add_last(p2)
            .build();

        let err = chain.chat(request()).await.unwrap_err();
        assert!(matches!(err, LLMError::SerializationError(_)));
    }

    #[tokio::test]
    async fn last_provider_error_propagates_even_if_fallback_condition() {
        // Even RateLimited should propagate when it comes from the last provider.
        let p1 = MockProvider::new("p1", vec![Err(LLMError::RateLimited("rl1".into()))]);
        let p2 = MockProvider::new("p2", vec![Err(LLMError::RateLimited("rl2".into()))]);

        let chain = FallbackChain::builder()
            .add(p1)
            .add_last(p2)
            .build();

        let err = chain.chat(request()).await.unwrap_err();
        assert!(matches!(err, LLMError::RateLimited(_)));
    }

    #[tokio::test]
    async fn health_check_true_if_any_healthy() {
        struct AlwaysHealthy;
        struct AlwaysUnhealthy;

        #[async_trait]
        impl LLMProvider for AlwaysHealthy {
            fn name(&self) -> &str { "healthy" }
            async fn chat(&self, _r: ChatCompletionRequest) -> LLMResult<ChatCompletionResponse> {
                unimplemented!()
            }
            async fn health_check(&self) -> LLMResult<bool> { Ok(true) }
        }

        #[async_trait]
        impl LLMProvider for AlwaysUnhealthy {
            fn name(&self) -> &str { "unhealthy" }
            async fn chat(&self, _r: ChatCompletionRequest) -> LLMResult<ChatCompletionResponse> {
                unimplemented!()
            }
            async fn health_check(&self) -> LLMResult<bool> { Ok(false) }
        }

        let chain = FallbackChain::builder()
            .add(AlwaysUnhealthy)
            .add_last(AlwaysHealthy)
            .build();

        assert!(chain.health_check().await.unwrap());
    }

    #[tokio::test]
    async fn health_check_false_if_all_unhealthy() {
        struct AlwaysUnhealthy;

        #[async_trait]
        impl LLMProvider for AlwaysUnhealthy {
            fn name(&self) -> &str { "unhealthy" }
            async fn chat(&self, _r: ChatCompletionRequest) -> LLMResult<ChatCompletionResponse> {
                unimplemented!()
            }
            async fn health_check(&self) -> LLMResult<bool> { Ok(false) }
        }

        let chain = FallbackChain::builder()
            .add(AlwaysUnhealthy)
            .add_last(AlwaysUnhealthy)
            .build();

        assert!(!chain.health_check().await.unwrap());
    }

    #[test]
    #[should_panic(expected = "at least one provider is required")]
    fn builder_panics_with_no_providers() {
        FallbackChain::builder().build();
    }

    #[test]
    fn chain_len_and_is_empty() {
        let chain = FallbackChain::builder()
            .add(MockProvider::new("p", vec![]))
            .build();
        assert_eq!(chain.len(), 1);
        assert!(!chain.is_empty());
    }
}
