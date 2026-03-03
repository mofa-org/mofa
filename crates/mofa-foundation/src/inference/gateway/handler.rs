//! Axum request handlers for the MoFA OpenAI-compatible inference gateway.
//!
//! Implements:
//! - `POST /v1/chat/completions` — non-streaming and SSE-streaming responses
//! - `GET  /v1/models`           — lists available models
//!
//! # Response headers
//!
//! Every response carries two extra headers:
//! - `X-MoFA-Backend`: where the request was actually routed (e.g., `local(qwen3)`)
//! - `X-MoFA-Latency-Ms`: end-to-end orchestrator latency in milliseconds

use std::convert::Infallible;
use std::net::IpAddr;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use axum::Json;
use axum::extract::{ConnectInfo, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode};
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::response::{IntoResponse, Response};
use futures::stream;

use crate::inference::orchestrator::InferenceOrchestrator;
use crate::inference::types::InferenceRequest;

use super::rate_limiter::TokenBucketLimiter;
use super::types::{
    ChatCompletionChunk, ChatCompletionRequest, ChatCompletionResponse, ChatMessage, Choice,
    ChunkChoice, Delta, GatewayErrorBody, ModelListResponse, ModelObject, Usage,
};

// ──────────────────────────────────────────────────────────────────────────────
// Shared application state
// ──────────────────────────────────────────────────────────────────────────────

/// Shared state injected into all axum handlers via `State<AppState>`.
#[derive(Clone)]
pub struct AppState {
    /// The inference orchestrator, protected for concurrent handler access.
    pub orchestrator: Arc<Mutex<InferenceOrchestrator>>,
    /// Per-IP token-bucket rate limiter.
    pub limiter: Arc<Mutex<TokenBucketLimiter>>,
    /// Models advertised on the `/v1/models` endpoint.
    pub available_models: Vec<String>,
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
fn estimate_tokens(s: &str) -> u32 {
    ((s.len() as f32) / 4.0).ceil() as u32
}

// ──────────────────────────────────────────────────────────────────────────────
// Rate-limit helper
// ──────────────────────────────────────────────────────────────────────────────

/// Check the rate limiter for `client_ip`.
///
/// Returns `None` if the request is allowed, or a `429` `Response` if the
/// bucket for this IP is exhausted.
fn check_rate_limit(
    limiter: &Arc<Mutex<TokenBucketLimiter>>,
    client_ip: IpAddr,
) -> Option<Response> {
    let allowed = limiter
        .lock()
        .map(|mut l| l.check_and_consume(client_ip))
        .unwrap_or(true); // if lock poisoned, allow to avoid false denials

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
    ConnectInfo(addr): ConnectInfo<std::net::SocketAddr>,
    Json(req): Json<ChatCompletionRequest>,
) -> Response {
    // ── Rate limit ────────────────────────────────────────────────────────────
    if let Some(denied) = check_rate_limit(&state.limiter, addr.ip()) {
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
    let result = state
        .orchestrator
        .lock()
        .map(|mut orch| orch.infer(&inference_req));

    let (result, latency_ms) = match result {
        Ok(r) => (r, start.elapsed().as_millis() as u64),
        Err(_) => {
            let err = GatewayErrorBody::server_error("orchestrator lock poisoned");
            return (StatusCode::INTERNAL_SERVER_ERROR, Json(err)).into_response();
        }
    };

    let backend_label = result.routed_to.to_string();
    let output_text = result.output.clone();
    let model_used = req.model.clone();
    let headers = mofa_headers(&backend_label, latency_ms);

    // ── Route to streaming or non-streaming path ──────────────────────────────
    if req.stream {
        build_streaming_response(output_text, model_used, headers)
    } else {
        build_nstream_response(output_text, model_used, prompt, headers)
    }
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

    let resp = ChatCompletionResponse {
        id: completion_id(),
        object: "chat.completion".to_string(),
        created: unix_now(),
        model,
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

    let mut response = Json(resp).into_response();
    response.headers_mut().extend(headers);
    response
}

// ──────────────────────────────────────────────────────────────────────────────
// SSE streaming response builder
// ──────────────────────────────────────────────────────────────────────────────

fn build_streaming_response(output: String, model: String, headers: HeaderMap) -> Response {
    let id = completion_id();
    let created = unix_now();

    // Split into word-level tokens to simulate real streaming.
    let words: Vec<String> = output.split_whitespace().map(|w| format!("{w} ")).collect();

    let mut chunks: Vec<ChatCompletionChunk> = Vec::new();

    // Role preamble chunk
    chunks.push(ChatCompletionChunk {
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
    });

    // Content chunks
    for word in words {
        chunks.push(ChatCompletionChunk {
            id: id.clone(),
            object: "chat.completion.chunk".to_string(),
            created,
            model: model.clone(),
            choices: vec![ChunkChoice {
                index: 0,
                delta: Delta {
                    role: None,
                    content: Some(word),
                },
                finish_reason: None,
            }],
        });
    }

    // Stop chunk
    chunks.push(ChatCompletionChunk {
        id: id.clone(),
        object: "chat.completion.chunk".to_string(),
        created,
        model: model.clone(),
        choices: vec![ChunkChoice {
            index: 0,
            delta: Delta::default(),
            finish_reason: Some("stop".to_string()),
        }],
    });

    // Convert chunks to SSE events + terminal [DONE] marker
    let events: Vec<Result<Event, Infallible>> = chunks
        .into_iter()
        .map(|c| {
            Ok::<_, Infallible>(
                Event::default().data(serde_json::to_string(&c).unwrap_or_default()),
            )
        })
        .chain(std::iter::once(Ok(Event::default().data("[DONE]"))))
        .collect();

    let stream = stream::iter(events);
    let mut sse_resp = Sse::new(stream)
        .keep_alive(KeepAlive::default())
        .into_response();

    sse_resp.headers_mut().extend(headers);
    sse_resp
}

// ──────────────────────────────────────────────────────────────────────────────
// Tests
// ──────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::inference::gateway::rate_limiter::TokenBucketLimiter;
    use crate::inference::orchestrator::{InferenceOrchestrator, OrchestratorConfig};
    use std::net::{IpAddr, Ipv4Addr};

    fn make_state(rpm: u32) -> AppState {
        let config = OrchestratorConfig::default();
        let orchestrator = Arc::new(Mutex::new(InferenceOrchestrator::new(config)));
        let limiter = Arc::new(Mutex::new(TokenBucketLimiter::new(rpm)));
        AppState {
            orchestrator,
            limiter,
            available_models: vec!["mofa-local".to_string(), "gpt-4o".to_string()],
        }
    }

    #[test]
    fn test_check_rate_limit_allows_within_budget() {
        let state = make_state(5);
        let ip = IpAddr::V4(Ipv4Addr::new(1, 2, 3, 4));
        for _ in 0..5 {
            assert!(check_rate_limit(&state.limiter, ip).is_none());
        }
    }

    #[test]
    fn test_check_rate_limit_rejects_over_budget() {
        let state = make_state(2);
        let ip = IpAddr::V4(Ipv4Addr::new(9, 9, 9, 9));
        check_rate_limit(&state.limiter, ip);
        check_rate_limit(&state.limiter, ip);
        let result = check_rate_limit(&state.limiter, ip);
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

    #[tokio::test]
    async fn test_streaming_response_ends_with_done() {
        use futures::StreamExt;
        // Build a minimal chunks list and collect SSE events
        let chunks: Vec<ChatCompletionChunk> = vec![ChatCompletionChunk {
            id: "test".into(),
            object: "chat.completion.chunk".into(),
            created: 0,
            model: "m".into(),
            choices: vec![ChunkChoice {
                index: 0,
                delta: Delta {
                    role: None,
                    content: Some("hi".into()),
                },
                finish_reason: None,
            }],
        }];

        let events: Vec<Result<Event, Infallible>> = chunks
            .into_iter()
            .map(|c| Ok::<_, Infallible>(Event::default().data(serde_json::to_string(&c).unwrap())))
            .chain(std::iter::once(Ok(Event::default().data("[DONE]"))))
            .collect();

        let stream = stream::iter(events);
        let all: Vec<_> = stream.collect().await;
        let last_event = all.last().unwrap().as_ref().unwrap();
        let dbg = format!("{last_event:?}");
        assert!(dbg.contains("[DONE]"), "stream must end with [DONE]: {dbg}");
    }
}
