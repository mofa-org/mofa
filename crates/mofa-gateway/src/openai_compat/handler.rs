//! Axum request handlers for the MoFA OpenAI-compatible inference gateway.
//!
//! Implements:
//! - `POST /v1/chat/completions` — non-streaming and SSE-streaming responses
//! - `GET  /v1/models`           — lists available models
//!
//! # Response headers
//!
//! Every response carries extra headers:
//! - `X-MoFA-Backend`: where the request was actually routed (e.g., `local(qwen3)`)
//! - `X-MoFA-Latency-Ms`: end-to-end orchestrator latency in milliseconds
//! - `X-MoFA-Cost-Usd`: estimated cost of the request in USD
//! - `X-MoFA-Tokens-In`: input tokens
//! - `X-MoFA-Tokens-Out`: output tokens

use std::net::IpAddr;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::Mutex;
use tokio::sync::RwLock;

use axum::{
    body::Body,
    extract::{ConnectInfo, State},
    http::{HeaderMap, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
    Json,
};

use crate::middleware::CostTracker;
use crate::streaming::SseBuilder;

use mofa_foundation::inference::orchestrator::InferenceOrchestrator;
use mofa_foundation::inference::types::InferenceRequest;

use super::rate_limiter::TokenBucketLimiter;
use super::types::{
    ChatCompletionRequest, ChatCompletionResponse, ChatMessage, Choice, GatewayErrorBody,
    ModelListResponse, ModelObject, Usage,
};

// ──────────────────────────────────────────────────────────────────────────────
// Shared application state
// ──────────────────────────────────────────────────────────────────────────────

/// Shared state injected into all axum handlers via `State<AppState>`.
#[derive(Clone)]
pub struct AppState {
    /// The inference orchestrator, protected for concurrent handler access.
    ///
    /// Uses `RwLock` so that read-only paths (e.g., `list_models`) do not
    /// contend with inference requests.
    pub orchestrator: Arc<RwLock<InferenceOrchestrator>>,
    /// Per-IP token-bucket rate limiter.
    pub limiter: Arc<Mutex<TokenBucketLimiter>>,
    /// Models advertised on the `/v1/models` endpoint.
    pub available_models: Vec<String>,
    /// Optional static API key for authentication.
    pub api_key: Option<String>,
}

// ──────────────────────────────────────────────────────────────────────────────
// Helpers
// ──────────────────────────────────────────────────────────────────────────────

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

/// Build the two MoFA-specific response headers.
fn mofa_headers(backend: &str, latency_ms: u64) -> HeaderMap {
    let mut headers = HeaderMap::new();
    if let Ok(v) = HeaderValue::from_str(backend) {
        headers.insert("x-mofa-backend", v);
    }
    if let Ok(v) = HeaderValue::from_str(&latency_ms.to_string()) {
        headers.insert("x-mofa-latency-ms", v);
    }
    headers
}

/// Estimate a rough token count (approx 4 chars per token).
///
/// Uses integer arithmetic to avoid f32 precision loss on strings
/// longer than ~16 MB (2^24 bytes, the f32 mantissa limit).
fn estimate_tokens(s: &str) -> u32 {
    // (len + 3) / 4 is the ceiling-division equivalent of (len as f32 / 4.0).ceil()
    u32::try_from((s.len() + 3) / 4).unwrap_or(u32::MAX)
}

// ──────────────────────────────────────────────────────────────────────────────
// Rate-limit helper
// ──────────────────────────────────────────────────────────────────────────────

/// Check the rate limiter for `client_ip`.
///
/// Returns `None` if the request is allowed, or a `429` `Response` if the
/// bucket for this IP is exhausted.
async fn check_rate_limit(
    limiter: &Arc<Mutex<TokenBucketLimiter>>,
    client_ip: IpAddr,
) -> Option<Response> {
    let allowed = {
        let mut l = limiter.lock().await;
        l.check_and_consume(client_ip)
    };

    if allowed {
        None
    } else {
        let body = GatewayErrorBody::rate_limited();
        let response = (StatusCode::TOO_MANY_REQUESTS, Json(body)).into_response();
        Some(response)
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// GET /v1/models
// ──────────────────────────────────────────────────────────────────────────────

/// Handler for `GET /v1/models`.
pub async fn list_models(State(state): State<AppState>) -> impl IntoResponse {
    let data: Vec<ModelObject> = state
        .available_models
        .iter()
        .map(|id| ModelObject {
            id: id.clone(),
            object: "model".to_string(),
            created: unix_now(),
            owned_by: "mofa".to_string(),
        })
        .collect();

    Json(ModelListResponse {
        object: "list".to_string(),
        data,
    })
}

// ──────────────────────────────────────────────────────────────────────────────
// POST /v1/chat/completions
// ──────────────────────────────────────────────────────────────────────────────

/// Handler for `POST /v1/chat/completions`.
///
/// Routes the request through the `InferenceOrchestrator` and returns either a
/// full JSON response or a Server-Sent Event stream depending on `stream`.
pub async fn chat_completions(
    State(state): State<AppState>,
    headers_map: HeaderMap,
    ConnectInfo(addr): ConnectInfo<std::net::SocketAddr>,
    Json(req): Json<ChatCompletionRequest>,
) -> Response {
    // ── Authentication ────────────────────────────────────────────────────────
    if let Some(expected_key) = &state.api_key {
        let auth_header = headers_map
            .get(axum::http::header::AUTHORIZATION)
            .and_then(|h| h.to_str().ok())
            .unwrap_or("");

        let provided_key = auth_header.strip_prefix("Bearer ").unwrap_or("").trim();

        // Constant-time comparison via `subtle` to prevent timing side-channel attacks.
        // Pad both values to equal length so no information is leaked about key length.
        use subtle::ConstantTimeEq;
        let max_len = provided_key.len().max(expected_key.len());
        let mut a = vec![0u8; max_len];
        let mut b = vec![0u8; max_len];
        a[..provided_key.len()].copy_from_slice(provided_key.as_bytes());
        b[..expected_key.len()].copy_from_slice(expected_key.as_bytes());
        let len_ok = (provided_key.len() == expected_key.len()) as u8;
        let keys_match = a.ct_eq(&b).unwrap_u8() & len_ok;

        if keys_match != 1 {
            let err = GatewayErrorBody::new("Invalid API key provided", "authentication_error");
            return (StatusCode::UNAUTHORIZED, Json(err)).into_response();
        }
    }

    // ── Rate limit ────────────────────────────────────────────────────────────
    if let Some(denied) = check_rate_limit(&state.limiter, addr.ip()).await {
        return denied;
    }

    // ── Validate ──────────────────────────────────────────────────────────────
    if req.messages.is_empty() {
        let err = GatewayErrorBody::invalid_request("messages must not be empty");
        return (StatusCode::BAD_REQUEST, Json(err)).into_response();
    }

    // ── Build InferenceRequest ────────────────────────────────────────────────
    let prompt = req.to_prompt();
    let inference_req =
        InferenceRequest::new(&req.model, &prompt, 7168).with_priority(req.priority());

    // ── Invoke orchestrator ───────────────────────────────────────────────────
    let start = Instant::now();
    if req.stream {
        let (result, token_stream) = {
            let mut orch = state.orchestrator.write().await;
            orch.infer_stream(&inference_req)
        };
        let latency_ms = start.elapsed().as_millis() as u64;

        let backend_label = result.routed_to.to_string();
        let model_used = req.model.clone();
        let headers = mofa_headers(&backend_label, latency_ms);

        build_streaming_response(token_stream, model_used, prompt, headers)
    } else {
        let result = {
            let mut orch = state.orchestrator.write().await;
            orch.infer(&inference_req)
        };
        let latency_ms = start.elapsed().as_millis() as u64;

        let backend_label = result.routed_to.to_string();
        let output_text = result.output.clone();
        let model_used = req.model.clone();
        let headers = mofa_headers(&backend_label, latency_ms);

        build_nstream_response(output_text, model_used, prompt, headers)
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// Cost tracking helper
// ──────────────────────────────────────────────────────────────────────────────

lazy_static::lazy_static! {
    static ref COST_TRACKER: CostTracker = CostTracker::new();
}

/// Build cost tracking headers for the response.
fn cost_headers(model: &str, input_tokens: u32, output_tokens: u32) -> HeaderMap {
    let pricing = COST_TRACKER.get_pricing(model);
    let cost = pricing.calculate_cost(input_tokens, output_tokens);
    
    let mut headers = HeaderMap::new();
    if let Ok(v) = HeaderValue::from_str(&format!("{:.6}", cost)) {
        headers.insert("x-mofa-cost-usd", v);
    }
    if let Ok(v) = HeaderValue::from_str(&input_tokens.to_string()) {
        headers.insert("x-mofa-tokens-in", v);
    }
    if let Ok(v) = HeaderValue::from_str(&output_tokens.to_string()) {
        headers.insert("x-mofa-tokens-out", v);
    }
    headers
}

// ──────────────────────────────────────────────────────────────────────────────
// Non-streaming response builder
// ──────────────────────────────────────────────────────────────────────────────

fn build_nstream_response(
    output: String,
    model: String,
    prompt: String,
    headers: HeaderMap,
) -> Response {
    let prompt_tokens = estimate_tokens(&prompt);
    let completion_tokens = estimate_tokens(&output);

    // Build cost headers
    let cost_headers = cost_headers(&model, prompt_tokens, completion_tokens);

    let resp = ChatCompletionResponse {
        id: completion_id(),
        object: "chat.completion".to_string(),
        created: unix_now(),
        model: model.clone(),
        choices: vec![Choice {
            index: 0,
            message: ChatMessage {
                role: "assistant".to_string(),
                content: output,
            },
            finish_reason: "stop".to_string(),
        }],
        usage: Usage {
            prompt_tokens,
            completion_tokens,
            total_tokens: prompt_tokens + completion_tokens,
        },
    };

    // Use pretty-printed JSON
    let json = serde_json::to_string_pretty(&resp).unwrap();
    let mut response = Response::new(Body::from(json));
    response.headers_mut().insert(
        "content-type",
        HeaderValue::from_static("application/json"),
    );
    response.headers_mut().extend(headers);
    response.headers_mut().extend(cost_headers);
    response
}

// ──────────────────────────────────────────────────────────────────────────────
// SSE streaming response builder
// ──────────────────────────────────────────────────────────────────────────────

/// Build an SSE streaming response from the orchestrator's [`BoxTokenStream`].
///
/// Delegates to [`SseBuilder`] which handles the full OpenAI SSE event sequence:
/// role chunk → content chunks → stop chunk → `[DONE]`.
fn build_streaming_response(
    token_stream: mofa_kernel::llm::streaming::BoxTokenStream,
    model: String,
    prompt: String,
    headers: HeaderMap,
) -> Response {
    // Estimate input tokens from prompt
    let input_tokens = estimate_tokens(&prompt);
    let output_tokens = 0u32; // Unknown until stream completes
    let cost_headers = cost_headers(&model, input_tokens, output_tokens);
    
    let mut all_headers = headers;
    all_headers.extend(cost_headers);
    
    SseBuilder::new(model)
        .with_headers(all_headers)
        .build_response(token_stream)
}

// ──────────────────────────────────────────────────────────────────────────────
// Tests
// ──────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::openai_compat::rate_limiter::TokenBucketLimiter;
    use mofa_foundation::inference::orchestrator::{InferenceOrchestrator, OrchestratorConfig};
    use std::net::{IpAddr, Ipv4Addr};

    fn make_state(rpm: u32) -> AppState {
        let config = OrchestratorConfig::default();
        let orchestrator = Arc::new(RwLock::new(InferenceOrchestrator::new(config)));
        let limiter = Arc::new(Mutex::new(TokenBucketLimiter::new(rpm)));
        AppState {
            orchestrator,
            limiter,
            available_models: vec!["mofa-local".to_string(), "gpt-4o".to_string()],
            api_key: None,
        }
    }

    fn make_state_with_auth(rpm: u32, key: &str) -> AppState {
        let mut state = make_state(rpm);
        state.api_key = Some(key.to_string());
        state
    }

    #[tokio::test]
    async fn test_check_rate_limit_allows_within_budget() {
        let state = make_state(5);
        let ip = IpAddr::V4(Ipv4Addr::new(1, 2, 3, 4));
        for _ in 0..5 {
            assert!(check_rate_limit(&state.limiter, ip).await.is_none());
        }
    }

    #[tokio::test]
    async fn test_check_rate_limit_rejects_over_budget() {
        let state = make_state(2);
        let ip = IpAddr::V4(Ipv4Addr::new(9, 9, 9, 9));
        check_rate_limit(&state.limiter, ip).await;
        check_rate_limit(&state.limiter, ip).await;
        let result = check_rate_limit(&state.limiter, ip).await;
        assert!(result.is_some(), "3rd request should be denied at 2 RPM");
    }

    #[test]
    fn test_non_streaming_response_shape() {
        let resp = build_nstream_response(
            "I am a helpful AI.".to_string(),
            "mofa-local".to_string(),
            "user: hello".to_string(),
            HeaderMap::new(),
        );
        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[test]
    fn test_mofa_headers_set_correctly() {
        let headers = mofa_headers("local(qwen3)", 42);
        assert!(headers.contains_key("x-mofa-backend"));
        assert!(headers.contains_key("x-mofa-latency-ms"));
        assert_eq!(
            headers.get("x-mofa-latency-ms").unwrap().to_str().unwrap(),
            "42"
        );
    }

    #[test]
    fn test_estimate_tokens() {
        assert!(estimate_tokens("Hello world") >= 2);
    }

    #[test]
    fn test_completion_id_unique() {
        let id1 = completion_id();
        std::thread::sleep(std::time::Duration::from_millis(2));
        let id2 = completion_id();
        assert_ne!(id1, id2);
    }

    #[test]
    fn test_cost_headers_present_in_response() {
        // Test that build_nstream_response includes cost headers
        let resp = build_nstream_response(
            "Hello world".to_string(),
            "gpt-4o".to_string(),
            "user: hi".to_string(),
            HeaderMap::new(),
        );

        // Check cost headers are present
        assert!(resp.headers().contains_key("x-mofa-cost-usd"), "x-mofa-cost-usd header missing");
        assert!(resp.headers().contains_key("x-mofa-tokens-in"), "x-mofa-tokens-in header missing");
        assert!(resp.headers().contains_key("x-mofa-tokens-out"), "x-mofa-tokens-out header missing");

        // Verify header values are valid
        let cost_usd = resp.headers().get("x-mofa-cost-usd").unwrap().to_str().unwrap();
        let tokens_in = resp.headers().get("x-mofa-tokens-in").unwrap().to_str().unwrap();
        let tokens_out = resp.headers().get("x-mofa-tokens-out").unwrap().to_str().unwrap();

        // Cost should be a valid number
        assert!(cost_usd.parse::<f64>().is_ok(), "Invalid cost value");
        // Tokens should be valid integers
        assert!(tokens_in.parse::<u32>().is_ok(), "Invalid tokens-in value");
        assert!(tokens_out.parse::<u32>().is_ok(), "Invalid tokens-out value");
    }

    #[tokio::test]
    async fn test_streaming_response_ends_with_done() {
        use axum::body::to_bytes;
        use mofa_kernel::llm::streaming::{StreamChunk, StreamError};
        use mofa_kernel::llm::types::FinishReason;

        // Build a BoxTokenStream with one content chunk and a stop chunk.
        let chunks: Vec<Result<StreamChunk, StreamError>> = vec![
            Ok(StreamChunk::text("hi")),
            Ok(StreamChunk::done(FinishReason::Stop)),
        ];
        let token_stream: mofa_kernel::llm::streaming::BoxTokenStream =
            Box::pin(futures::stream::iter(chunks));

        let resp = build_streaming_response(
            token_stream, 
            "test-model".to_string(), 
            "test prompt".to_string(), 
            HeaderMap::new()
        );

        // Collect SSE body and check that it ends with [DONE]
        let bytes = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let text = String::from_utf8(bytes.to_vec()).unwrap();
        let data_lines: Vec<&str> = text
            .lines()
            .filter_map(|l| l.strip_prefix("data: "))
            .collect();

        assert!(!data_lines.is_empty(), "Expected SSE events");
        assert_eq!(
            *data_lines.last().unwrap(),
            "[DONE]",
            "stream must end with [DONE]"
        );
    }

    #[tokio::test]
    async fn test_auth_failure_with_wrong_key() {
        let state = make_state_with_auth(10, "secret-key");
        let mut headers = HeaderMap::new();
        headers.insert(
            axum::http::header::AUTHORIZATION,
            "Bearer wrong-key".parse().unwrap(),
        );

        let req = ChatCompletionRequest {
            model: "test".to_string(),
            messages: vec![ChatMessage {
                role: "user".to_string(),
                content: "hi".to_string(),
            }],
            stream: false,
            priority: crate::openai_compat::types::RequestPriorityParam::Normal,
            max_tokens: None,
            temperature: None,
        };

        let addr = std::net::SocketAddr::from(([127, 0, 0, 1], 8080));
        let resp = chat_completions(State(state), headers, ConnectInfo(addr), Json(req)).await;

        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn test_auth_success() {
        let state = make_state_with_auth(10, "secret-key");
        let mut headers = HeaderMap::new();
        // Test with Bearer prefix
        headers.insert(
            axum::http::header::AUTHORIZATION,
            "Bearer secret-key".parse().unwrap(),
        );

        let req = ChatCompletionRequest {
            model: "test".to_string(),
            messages: vec![ChatMessage {
                role: "user".to_string(),
                content: "hi".to_string(),
            }],
            stream: false,
            priority: crate::openai_compat::types::RequestPriorityParam::Normal,
            max_tokens: None,
            temperature: None,
        };

        let addr = std::net::SocketAddr::from(([127, 0, 0, 1], 8080));
        let resp = chat_completions(State(state), headers, ConnectInfo(addr), Json(req)).await;

        // It should reach orchestrator error if we don't mock it, but NOT auth error
        assert_ne!(resp.status(), StatusCode::UNAUTHORIZED);
    }
}
