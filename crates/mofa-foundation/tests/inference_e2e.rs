//! End-to-end integration tests for the `mofa-foundation` inference pipeline.
//!
//! These tests exercise the inference integration path using
//! [`common::mock_provider::MockLLMProvider`] as a deterministic stand-in for
//! a real LLM backend.  They establish the test conventions and helper patterns
//! used throughout Phases 1–4 of the Cognitive Compute Mesh implementation: all
//! later PRs (IRP, SRP, CDP, HCP adapters) can immediately add protocol-specific
//! assertions inside these categories without scaffolding overhead.
//!
//! # Running
//!
//! ```bash
//! # Run only this integration test binary
//! cargo test -p mofa-foundation --test inference_e2e
//!
//! # Run with output visible (useful during development)
//! cargo test -p mofa-foundation --test inference_e2e -- --nocapture
//! ```
//!
//! # Adding new tests
//!
//! 1. Pick the appropriate section below (chat / streaming / embedding / health /
//!    capability / observability).
//! 2. Write an `async fn` decorated with `#[tokio::test]`.
//! 3. Build a [`MockLLMProvider`] via [`MockLLMProvider::builder()`], set up the
//!    expected responses, call the trait method under test, then assert.
//!
//! If you need a behaviour not yet exposed by the builder, add it to
//! [`common::mock_provider`] and update this comment.

mod common;

use common::mock_provider::MockLLMProvider;

use mofa_kernel::agent::AgentError;
use mofa_kernel::llm::{
    ChatCompletionRequest, EmbeddingInput, EmbeddingRequest, LLMProvider,
};

// ─────────────────────────────────────────────────────────────────────────────
// § 1  Chat — happy path
// ─────────────────────────────────────────────────────────────────────────────

/// A freshly-built mock with no queued responses returns the default canned
/// string `"This is a mock response."`.
#[tokio::test]
async fn chat_returns_default_canned_response_when_queue_is_empty() {
    let mock = MockLLMProvider::builder().build();

    let response = mock
        .chat(ChatCompletionRequest::new("mock-model").user("Hello"))
        .await
        .expect("chat should succeed with default response");

    assert_eq!(
        response.content(),
        Some("This is a mock response."),
        "default response text mismatch"
    );
}

/// Every call to `chat()` is appended to the call log in invocation order.
#[tokio::test]
async fn chat_records_every_call_in_invocation_order() {
    let mock = MockLLMProvider::builder().build();

    mock.chat(ChatCompletionRequest::new("m").user("first")).await.unwrap();
    mock.chat(ChatCompletionRequest::new("m").user("second")).await.unwrap();

    assert_eq!(mock.chat_call_count(), 2);

    let calls = mock.chat_calls();
    assert_eq!(
        calls[0].messages.last().and_then(|m| m.text_content()),
        Some("first"),
        "first recorded request mismatch"
    );
    assert_eq!(
        calls[1].messages.last().and_then(|m| m.text_content()),
        Some("second"),
        "second recorded request mismatch"
    );
}

/// Queued responses are consumed in FIFO order; the fallback default fires once
/// the queue is exhausted.
#[tokio::test]
async fn chat_consumes_queued_responses_in_fifo_order() {
    let mock = MockLLMProvider::builder()
        .respond_with("alpha")
        .respond_with("beta")
        .build();

    let r1 = mock.chat(ChatCompletionRequest::new("m").user("q1")).await.unwrap();
    let r2 = mock.chat(ChatCompletionRequest::new("m").user("q2")).await.unwrap();
    // Third call — queue is empty, fallback fires.
    let r3 = mock.chat(ChatCompletionRequest::new("m").user("q3")).await.unwrap();

    assert_eq!(r1.content(), Some("alpha"));
    assert_eq!(r2.content(), Some("beta"));
    assert_eq!(r3.content(), Some("This is a mock response."), "fallback not fired");
}

/// `last_chat_request()` returns the most recent request, or `None` if never
/// called.
#[tokio::test]
async fn last_chat_request_reflects_most_recent_call() {
    let mock = MockLLMProvider::builder().build();

    assert!(mock.last_chat_request().is_none(), "should be None before any call");

    mock.chat(ChatCompletionRequest::new("m").user("first")).await.unwrap();
    mock.chat(ChatCompletionRequest::new("m").user("last")).await.unwrap();

    let last = mock.last_chat_request().expect("should be Some after calls");
    assert_eq!(
        last.messages.last().and_then(|m| m.text_content()),
        Some("last")
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// § 2  Chat — error path (explicit errors, not silent failures)
// ─────────────────────────────────────────────────────────────────────────────

/// When an error is queued the provider surfaces it as a typed `AgentError`
/// rather than silently swallowing it.
#[tokio::test]
async fn chat_surfaces_queued_error_as_typed_agent_error() {
    let mock = MockLLMProvider::builder()
        .respond_with_error(AgentError::Other("simulated backend failure".to_string()))
        .build();

    let result = mock.chat(ChatCompletionRequest::new("m").user("hi")).await;

    assert!(result.is_err(), "expected Err when an error is queued");
    match result.unwrap_err() {
        AgentError::Other(msg) => {
            assert!(msg.contains("simulated backend failure"), "error message mismatch: {msg}")
        }
        other => panic!("unexpected error variant: {other:?}"),
    }
    // The failed call is still recorded.
    assert_eq!(mock.chat_call_count(), 1);
}

/// After a queued error is consumed the provider recovers to its default
/// success response on the next call.
#[tokio::test]
async fn chat_recovers_to_default_after_queued_error_is_consumed() {
    let mock = MockLLMProvider::builder()
        .respond_with_error(AgentError::Other("transient failure".to_string()))
        .build();

    // First call — error consumed.
    assert!(mock.chat(ChatCompletionRequest::new("m").user("fail")).await.is_err());
    // Second call — queue empty, default success.
    let resp = mock.chat(ChatCompletionRequest::new("m").user("ok")).await.unwrap();
    assert_eq!(resp.content(), Some("This is a mock response."));
    assert_eq!(mock.chat_call_count(), 2);
}

// ─────────────────────────────────────────────────────────────────────────────
// § 3  Streaming
// ─────────────────────────────────────────────────────────────────────────────

/// A streaming-enabled mock emits the queued tokens in order, followed by a
/// terminal `Stop` chunk.
#[tokio::test]
async fn chat_stream_emits_queued_tokens_in_order() {
    use futures::StreamExt;

    let tokens: Vec<String> = vec!["Hello".into(), ", ".into(), "world".into(), "!".into()];
    let mock = MockLLMProvider::builder()
        .with_streaming(true)
        .stream_with_tokens(tokens.iter().map(String::from).collect())
        .build();

    let mut stream = mock
        .chat_stream(ChatCompletionRequest::new("m").user("go"))
        .await
        .expect("stream should open successfully");

    let mut collected: Vec<String> = Vec::new();
    while let Some(chunk_result) = stream.next().await {
        let chunk = chunk_result.expect("each chunk should be Ok");
        for choice in &chunk.choices {
            if let Some(content) = &choice.delta.content {
                collected.push(content.clone());
            }
        }
    }

    assert_eq!(collected, tokens, "streamed token sequence mismatch");
    assert_eq!(mock.stream_call_count(), 1);
}

/// When streaming is disabled `chat_stream()` returns a typed error immediately
/// (before any stream is opened) rather than panicking or producing garbage.
#[tokio::test]
async fn chat_stream_returns_typed_error_when_streaming_disabled() {
    let mock = MockLLMProvider::builder().with_streaming(false).build();

    let result = mock
        .chat_stream(ChatCompletionRequest::new("m").user("hi"))
        .await;

    assert!(result.is_err(), "should refuse when streaming is disabled");
    // The refused call is NOT logged (the provider never accepted it).
    assert_eq!(mock.stream_call_count(), 0, "refused stream should not be recorded");
}

/// A queued streaming error is yielded as `Err(AgentError)` within the stream
/// rather than propagating as a hard panic.
#[tokio::test]
async fn chat_stream_surfaces_queued_error_as_stream_item() {
    use futures::StreamExt;

    let mock = MockLLMProvider::builder()
        .with_streaming(true)
        .stream_with_error(AgentError::Other("stream content failure".to_string()))
        .build();

    // Opening the stream succeeds — the error lives inside the stream.
    let mut stream = mock
        .chat_stream(ChatCompletionRequest::new("m").user("go"))
        .await
        .expect("stream opening should succeed even when content will fail");

    let first = stream.next().await.expect("stream must yield at least one item");
    assert!(first.is_err(), "first item should carry the queued error");
    match first.unwrap_err() {
        AgentError::Other(msg) => {
            assert!(msg.contains("stream content failure"), "error message mismatch: {msg}")
        }
        other => panic!("unexpected error variant: {other:?}"),
    }
    // The call IS recorded — the connection was opened.
    assert_eq!(mock.stream_call_count(), 1);
}

// ─────────────────────────────────────────────────────────────────────────────
// § 4  Embedding
// ─────────────────────────────────────────────────────────────────────────────

/// A provider that does not declare embedding support returns a typed error
/// before consulting the response queue, and the call is not recorded.
#[tokio::test]
async fn embedding_returns_typed_error_when_not_supported() {
    let mock = MockLLMProvider::builder().with_embedding(false).build();

    let result = mock
        .embedding(EmbeddingRequest {
            model: "text-embedding-3-small".to_string(),
            input: EmbeddingInput::Single("hello world".to_string()),
        })
        .await;

    assert!(result.is_err(), "embedding should be refused when unsupported");
    assert_eq!(mock.embedding_call_count(), 0, "refused call should not be recorded");
}

/// When embedding is enabled and a vector is queued, the correct values are
/// returned and the call is recorded.
#[tokio::test]
async fn embedding_returns_queued_vector_when_supported() {
    let expected: Vec<f32> = vec![0.1, 0.2, 0.3, 0.4];
    let mock = MockLLMProvider::builder()
        .with_embedding(true)
        .embedding_responds_with(expected.clone())
        .build();

    let response = mock
        .embedding(EmbeddingRequest {
            model: "text-embedding-3-small".to_string(),
            input: EmbeddingInput::Single("hello world".to_string()),
        })
        .await
        .expect("embedding should succeed when supported and a vector is queued");

    let returned_vec = &response.data.first().expect("at least one data entry").embedding;
    assert_eq!(returned_vec, &expected, "returned embedding vector mismatch");
    assert_eq!(mock.embedding_call_count(), 1);
}

/// When embedding is enabled but the queue is empty a zero-vector sentinel is
/// returned rather than an error.
#[tokio::test]
async fn embedding_falls_back_to_zero_vector_when_queue_is_empty() {
    let mock = MockLLMProvider::builder().with_embedding(true).build();

    let response = mock
        .embedding(EmbeddingRequest {
            model: "text-embedding-3-small".to_string(),
            input: EmbeddingInput::Single("default fallback".to_string()),
        })
        .await
        .expect("fallback embedding should not error");

    let vec = &response.data.first().expect("data must be non-empty").embedding;
    assert!(!vec.is_empty(), "fallback embedding vector should not be empty");
    assert!(
        vec.iter().all(|&v| v == 0.0),
        "fallback embedding values should all be zero"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// § 5  Health check
// ─────────────────────────────────────────────────────────────────────────────

/// Default health check returns `Ok(true)` and increments the call counter.
#[tokio::test]
async fn health_check_defaults_to_ok_true() {
    let mock = MockLLMProvider::builder().build();

    let healthy = mock.health_check().await.expect("health check must not error by default");
    assert!(healthy, "default health check result must be true");
    assert_eq!(mock.health_check_call_count(), 1);
}

/// A queued `Ok(false)` reports the provider as unhealthy.
#[tokio::test]
async fn health_check_can_report_unhealthy_via_queued_false() {
    let mock = MockLLMProvider::builder().health_responds_with(Ok(false)).build();

    let healthy = mock.health_check().await.unwrap();
    assert!(!healthy, "queued Ok(false) should report unhealthy");
}

/// After the queued responses are consumed, health check reverts to `Ok(true)`.
#[tokio::test]
async fn health_check_reverts_to_ok_true_after_queue_exhausted() {
    let mock = MockLLMProvider::builder().health_responds_with(Ok(false)).build();

    assert!(!mock.health_check().await.unwrap(), "first — queued false");
    assert!(mock.health_check().await.unwrap(), "second — fallback true");
    assert_eq!(mock.health_check_call_count(), 2);
}

/// A queued error is surfaced as a typed `AgentError`.
#[tokio::test]
async fn health_check_surfaces_queued_error() {
    let mock = MockLLMProvider::builder()
        .health_responds_with(Err(AgentError::Other("health probe failed".to_string())))
        .build();

    let result = mock.health_check().await;
    assert!(result.is_err(), "queued health error should propagate");
    match result.unwrap_err() {
        AgentError::Other(msg) => {
            assert!(msg.contains("health probe failed"), "error message mismatch: {msg}")
        }
        other => panic!("unexpected error variant: {other:?}"),
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// § 6  Capability flags
// ─────────────────────────────────────────────────────────────────────────────

/// All capability flags and metadata set on the builder are surfaced correctly
/// through the standard `LLMProvider` interface.
#[tokio::test]
async fn capability_flags_reflect_builder_configuration() {
    let mock = MockLLMProvider::builder()
        .with_name("custom-backend")
        .with_default_model("custom-model-7b")
        .with_streaming(false)
        .with_embedding(true)
        .with_tools(false)
        .with_vision(true)
        .build();

    assert_eq!(mock.name(), "custom-backend");
    assert_eq!(mock.default_model(), "custom-model-7b");
    assert!(!mock.supports_streaming(), "streaming should be disabled");
    assert!(mock.supports_embedding(), "embedding should be enabled");
    assert!(!mock.supports_tools(), "tools should be disabled");
    assert!(mock.supports_vision(), "vision should be enabled");
    assert!(
        mock.supported_models().contains(&"custom-model-7b"),
        "supported_models should include the default model"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// § 7  Observability / `was_called()`
// ─────────────────────────────────────────────────────────────────────────────

/// `was_called()` is `false` before any method is invoked, and `true` after
/// any single method call.
#[tokio::test]
async fn was_called_transitions_from_false_to_true_on_first_invocation() {
    let mock = MockLLMProvider::builder().build();

    assert!(!mock.was_called(), "fresh mock should report uncalled");

    mock.chat(ChatCompletionRequest::new("m").user("ping")).await.unwrap();

    assert!(mock.was_called(), "should be marked called after chat()");
}
