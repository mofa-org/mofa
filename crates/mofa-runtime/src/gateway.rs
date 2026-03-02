//! OpenAI-compatible gateway for MoFA runtime.
//!
//! First-scope support includes:
//! - `POST /v1/chat/completions` non-streaming passthrough
//! - weighted backend routing with fallback
//! - fixed-window rate limiting
//! - request correlation id propagation (`x-request-id`)

use axum::{
    Json, Router,
    extract::State,
    http::{HeaderMap, HeaderValue, StatusCode},
    response::IntoResponse,
    routing::{get, post},
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    cmp::Reverse,
    net::SocketAddr,
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::sync::{Mutex, RwLock};
use tracing::{error, info, warn};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GatewayConfig {
    pub host: String,
    pub port: u16,
    pub backends: Vec<BackendConfig>,
    pub rate_limit: RateLimitConfig,
    pub request_timeout_ms: u64,
}

impl GatewayConfig {
    pub fn new(host: impl Into<String>, port: u16, backends: Vec<BackendConfig>) -> Self {
        Self {
            host: host.into(),
            port,
            backends,
            rate_limit: RateLimitConfig::default(),
            request_timeout_ms: 15_000,
        }
    }

    pub fn socket_addr(&self) -> anyhow::Result<SocketAddr> {
        Ok(format!("{}:{}", self.host, self.port).parse()?)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackendConfig {
    pub name: String,
    pub base_url: String,
    pub weight: u32,
    pub enabled: bool,
}

impl BackendConfig {
    pub fn new(name: impl Into<String>, base_url: impl Into<String>, weight: u32) -> Self {
        Self {
            name: name.into(),
            base_url: base_url.into(),
            weight,
            enabled: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RateLimitConfig {
    pub requests_per_minute: u32,
}

impl Default for RateLimitConfig {
    fn default() -> Self {
        Self {
            requests_per_minute: 120,
        }
    }
}

#[derive(Debug)]
struct BackendState {
    config: BackendConfig,
    healthy: bool,
    last_failure: Option<Instant>,
}

#[derive(Debug)]
struct FixedWindowRateLimiter {
    window_start: Instant,
    count: u32,
    max_per_minute: u32,
}

impl FixedWindowRateLimiter {
    fn new(max_per_minute: u32) -> Self {
        Self {
            window_start: Instant::now(),
            count: 0,
            max_per_minute,
        }
    }

    fn allow(&mut self) -> bool {
        let now = Instant::now();
        if now.duration_since(self.window_start) >= Duration::from_secs(60) {
            self.window_start = now;
            self.count = 0;
        }
        if self.count >= self.max_per_minute {
            return false;
        }
        self.count += 1;
        true
    }
}

#[derive(Clone)]
struct GatewayState {
    client: reqwest::Client,
    backends: Arc<RwLock<Vec<BackendState>>>,
    limiter: Arc<Mutex<FixedWindowRateLimiter>>,
}

pub fn build_router(config: GatewayConfig) -> anyhow::Result<Router> {
    if config.backends.is_empty() {
        anyhow::bail!("gateway requires at least one backend");
    }
    let client = reqwest::Client::builder()
        .timeout(Duration::from_millis(config.request_timeout_ms))
        .build()?;

    let backends = config
        .backends
        .into_iter()
        .filter(|b| b.enabled)
        .map(|b| BackendState {
            config: b,
            healthy: true,
            last_failure: None,
        })
        .collect::<Vec<_>>();
    if backends.is_empty() {
        anyhow::bail!("all gateway backends are disabled");
    }

    let state = GatewayState {
        client,
        backends: Arc::new(RwLock::new(backends)),
        limiter: Arc::new(Mutex::new(FixedWindowRateLimiter::new(
            config.rate_limit.requests_per_minute,
        ))),
    };

    Ok(Router::new()
        .route("/health", get(health_handler))
        .route("/v1/chat/completions", post(chat_completions_handler))
        .with_state(state))
}

pub async fn run_gateway(config: GatewayConfig) -> anyhow::Result<()> {
    let addr = config.socket_addr()?;
    let app = build_router(config)?;
    let listener = tokio::net::TcpListener::bind(addr).await?;
    info!("Gateway listening on {}", addr);
    axum::serve(listener, app).await?;
    Ok(())
}

async fn health_handler(State(state): State<GatewayState>) -> impl IntoResponse {
    let backends = state.backends.read().await;
    let data = backends
        .iter()
        .map(|b| {
            json!({
                "name": b.config.name,
                "base_url": b.config.base_url,
                "weight": b.config.weight,
                "healthy": b.healthy
            })
        })
        .collect::<Vec<_>>();

    (
        StatusCode::OK,
        Json(json!({ "status": "ok", "backends": data })),
    )
}

async fn chat_completions_handler(
    State(state): State<GatewayState>,
    headers: HeaderMap,
    Json(payload): Json<Value>,
) -> impl IntoResponse {
    let request_id = headers
        .get("x-request-id")
        .and_then(|v| v.to_str().ok())
        .map(ToString::to_string)
        .unwrap_or_else(|| Uuid::new_v4().to_string());

    if !is_valid_chat_request(&payload) {
        return error_response(
            StatusCode::BAD_REQUEST,
            "invalid_request",
            "Request must include `model` and non-empty `messages`",
            &request_id,
        );
    }

    {
        let mut limiter = state.limiter.lock().await;
        if !limiter.allow() {
            return error_response(
                StatusCode::TOO_MANY_REQUESTS,
                "rate_limited",
                "Rate limit exceeded",
                &request_id,
            );
        }
    }

    let backend_order = select_backends(&state).await;
    if backend_order.is_empty() {
        return error_response(
            StatusCode::BAD_GATEWAY,
            "no_backend",
            "No healthy backend available",
            &request_id,
        );
    }

    for backend_idx in backend_order {
        let backend_url = {
            let backends = state.backends.read().await;
            let base = backends[backend_idx].config.base_url.trim_end_matches('/');
            format!("{}/v1/chat/completions", base)
        };

        match state
            .client
            .post(&backend_url)
            .header("x-request-id", request_id.as_str())
            .json(&payload)
            .send()
            .await
        {
            Ok(resp) if resp.status().is_success() => match resp.json::<Value>().await {
                Ok(body) => {
                    let mut resp_headers = HeaderMap::new();
                    if let Ok(hv) = HeaderValue::from_str(&request_id) {
                        resp_headers.insert("x-request-id", hv);
                    }
                    return (StatusCode::OK, resp_headers, Json(body)).into_response();
                }
                Err(e) => {
                    error!("backend returned invalid JSON: {}", e);
                    mark_backend_unhealthy(&state, backend_idx).await;
                }
            },
            Ok(resp) => {
                warn!("backend returned status {}", resp.status());
                mark_backend_unhealthy(&state, backend_idx).await;
            }
            Err(e) => {
                warn!("backend request failed: {}", e);
                mark_backend_unhealthy(&state, backend_idx).await;
            }
        }
    }

    error_response(
        StatusCode::BAD_GATEWAY,
        "backend_failure",
        "All backends failed",
        &request_id,
    )
}

fn is_valid_chat_request(payload: &Value) -> bool {
    let stream_requested = payload
        .get("stream")
        .and_then(Value::as_bool)
        .unwrap_or(false);

    if stream_requested {
        return false;
    }

    payload
        .get("model")
        .and_then(Value::as_str)
        .map(|m| !m.trim().is_empty())
        .unwrap_or(false)
        && payload
            .get("messages")
            .and_then(Value::as_array)
            .map(|m| !m.is_empty())
            .unwrap_or(false)
}

fn error_response(
    status: StatusCode,
    code: &str,
    message: &str,
    request_id: &str,
) -> axum::response::Response {
    let mut headers = HeaderMap::new();
    if let Ok(hv) = HeaderValue::from_str(request_id) {
        headers.insert("x-request-id", hv);
    }
    if status == StatusCode::TOO_MANY_REQUESTS {
        headers.insert("retry-after", HeaderValue::from_static("60"));
    }
    (
        status,
        headers,
        Json(json!({
            "error": {
                "code": code,
                "message": message
            }
        })),
    )
        .into_response()
}

async fn select_backends(state: &GatewayState) -> Vec<usize> {
    let now = Instant::now();
    let mut backends = state.backends.write().await;
    for backend in backends.iter_mut() {
        // auto-heal backend after cooldown window
        if !backend.healthy
            && backend
                .last_failure
                .map(|t| now.duration_since(t) >= Duration::from_secs(10))
                .unwrap_or(false)
        {
            backend.healthy = true;
        }
    }

    let mut indexes = backends
        .iter()
        .enumerate()
        .filter(|(_, b)| b.healthy)
        .map(|(i, b)| (i, b.config.weight))
        .collect::<Vec<_>>();

    if indexes.is_empty() {
        indexes = backends
            .iter()
            .enumerate()
            .map(|(i, b)| (i, b.config.weight))
            .collect::<Vec<_>>();
    }
    indexes.sort_by_key(|(_, weight)| Reverse(*weight));
    indexes.into_iter().map(|(i, _)| i).collect()
}

async fn mark_backend_unhealthy(state: &GatewayState, index: usize) {
    let mut backends = state.backends.write().await;
    if let Some(backend) = backends.get_mut(index) {
        backend.healthy = false;
        backend.last_failure = Some(Instant::now());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::routing::post;
    use std::sync::atomic::{AtomicBool, Ordering};

    async fn start_mock_backend(status: StatusCode, body: Value) -> String {
        let app = Router::new().route(
            "/v1/chat/completions",
            post({
                let body = body.clone();
                move || async move { (status, Json(body.clone())) }
            }),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        format!("http://{}", addr)
    }

    async fn start_mock_backend_with_header_capture(
        status: StatusCode,
        body: Value,
        saw_request_id: Arc<AtomicBool>,
    ) -> String {
        let app = Router::new().route(
            "/v1/chat/completions",
            post({
                let body = body.clone();
                let saw_request_id = saw_request_id.clone();
                move |headers: HeaderMap| {
                    let body = body.clone();
                    let saw_request_id = saw_request_id.clone();
                    async move {
                        if headers.contains_key("x-request-id") {
                            saw_request_id.store(true, Ordering::SeqCst);
                        }
                        (status, Json(body))
                    }
                }
            }),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        format!("http://{}", addr)
    }

    async fn start_gateway(config: GatewayConfig) -> String {
        let app = build_router(config).unwrap();
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        format!("http://{}", addr)
    }

    fn valid_payload() -> Value {
        json!({
            "model": "gpt-4o-mini",
            "messages": [{"role":"user", "content":"hello"}]
        })
    }

    #[tokio::test]
    async fn test_rejects_invalid_request() {
        let backend = start_mock_backend(StatusCode::OK, json!({"ok": true})).await;
        let gateway = start_gateway(GatewayConfig::new(
            "127.0.0.1",
            0,
            vec![BackendConfig::new("b1", backend, 1)],
        ))
        .await;

        let client = reqwest::Client::new();
        let resp = client
            .post(format!("{}/v1/chat/completions", gateway))
            .json(&json!({"messages":[]}))
            .send()
            .await
            .unwrap();

        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn test_rejects_streaming_request_in_v1_scope() {
        let backend = start_mock_backend(StatusCode::OK, json!({"ok": true})).await;
        let gateway = start_gateway(GatewayConfig::new(
            "127.0.0.1",
            0,
            vec![BackendConfig::new("b1", backend, 1)],
        ))
        .await;

        let client = reqwest::Client::new();
        let resp = client
            .post(format!("{}/v1/chat/completions", gateway))
            .json(&json!({
                "model":"gpt-4o-mini",
                "messages":[{"role":"user","content":"hello"}],
                "stream": true
            }))
            .send()
            .await
            .unwrap();

        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn test_fallback_to_secondary_backend() {
        let primary = start_mock_backend(
            StatusCode::INTERNAL_SERVER_ERROR,
            json!({"error": "primary down"}),
        )
        .await;
        let secondary = start_mock_backend(
            StatusCode::OK,
            json!({
                "id": "chatcmpl-1",
                "object":"chat.completion",
                "created": 1,
                "model":"gpt-4o-mini",
                "choices":[{"index":0, "message":{"role":"assistant","content":"ok"}, "finish_reason":"stop"}]
            }),
        )
        .await;

        let gateway = start_gateway(GatewayConfig::new(
            "127.0.0.1",
            0,
            vec![
                BackendConfig::new("primary", primary, 10),
                BackendConfig::new("secondary", secondary, 5),
            ],
        ))
        .await;

        let client = reqwest::Client::new();
        let resp = client
            .post(format!("{}/v1/chat/completions", gateway))
            .json(&valid_payload())
            .send()
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let body = resp.json::<Value>().await.unwrap();
        assert_eq!(body["object"], "chat.completion");
    }

    #[tokio::test]
    async fn test_rate_limit_enforced() {
        let backend = start_mock_backend(StatusCode::OK, json!({"ok": true})).await;
        let mut cfg =
            GatewayConfig::new("127.0.0.1", 0, vec![BackendConfig::new("b1", backend, 1)]);
        cfg.rate_limit.requests_per_minute = 1;
        let gateway = start_gateway(cfg).await;

        let client = reqwest::Client::new();
        let first = client
            .post(format!("{}/v1/chat/completions", gateway))
            .json(&valid_payload())
            .send()
            .await
            .unwrap();
        assert_eq!(first.status(), StatusCode::OK);

        let second = client
            .post(format!("{}/v1/chat/completions", gateway))
            .json(&valid_payload())
            .send()
            .await
            .unwrap();
        assert_eq!(second.status(), StatusCode::TOO_MANY_REQUESTS);
        assert!(second.headers().contains_key("retry-after"));
    }

    #[tokio::test]
    async fn test_request_id_header_present() {
        let backend = start_mock_backend(StatusCode::OK, json!({"ok": true})).await;
        let gateway = start_gateway(GatewayConfig::new(
            "127.0.0.1",
            0,
            vec![BackendConfig::new("b1", backend, 1)],
        ))
        .await;

        let client = reqwest::Client::new();
        let resp = client
            .post(format!("{}/v1/chat/completions", gateway))
            .json(&valid_payload())
            .send()
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        assert!(resp.headers().contains_key("x-request-id"));
    }

    #[tokio::test]
    async fn test_request_id_propagated_to_backend() {
        let flag = Arc::new(AtomicBool::new(false));
        let backend = start_mock_backend_with_header_capture(
            StatusCode::OK,
            json!({"ok": true}),
            flag.clone(),
        )
        .await;
        let gateway = start_gateway(GatewayConfig::new(
            "127.0.0.1",
            0,
            vec![BackendConfig::new("b1", backend, 1)],
        ))
        .await;

        let client = reqwest::Client::new();
        let resp = client
            .post(format!("{}/v1/chat/completions", gateway))
            .json(&valid_payload())
            .send()
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        assert!(flag.load(Ordering::SeqCst));
    }
}
