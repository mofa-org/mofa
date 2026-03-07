//! In-process gateway test harness.
//!
//! [`GatewayTestHarness`] spins up a real axum HTTP server that replicates the
//! gateway's behaviour — auth enforcement, per-route rate limiting, per-route
//! deadline enforcement, and HTTP proxy dispatch to mock agent backends — all
//! on a randomly-assigned port so tests never collide.
//!
//! The harness implements [`Drop`] to send a graceful shutdown signal so tests
//! do not leak background tasks.

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use axum::body::Body;
use axum::extract::{Path, State};
use axum::http::{HeaderMap, Request, StatusCode};
use axum::response::IntoResponse;
use axum::{Json, Router};
use axum::routing::{any, delete, get, post};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tokio::net::TcpListener;
use tokio::sync::{RwLock, oneshot};

use mofa_gateway::middleware::rate_limit::RateLimiter;

// ─────────────────────────────────────────────────────────────────────────────
// Route entry
// ─────────────────────────────────────────────────────────────────────────────

/// A route registered in the test harness.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HarnessRoute {
    pub id: String,
    pub path_pattern: String,
    pub backend_url: String,
    pub method: String,
    pub enabled: bool,
    /// Optional per-route request timeout in milliseconds.
    pub timeout_ms: Option<u64>,
    /// Optional per-route rate limit: maximum requests per second.
    pub max_requests_per_sec: Option<u64>,
}

/// Request body for `POST /admin/routes` in the test harness.
#[derive(Debug, Deserialize)]
pub struct RegisterRouteBody {
    pub id: String,
    pub path_pattern: String,
    pub backend_url: String,
    pub method: String,
    #[serde(default = "default_true")]
    pub enabled: bool,
    pub timeout_ms: Option<u64>,
    pub max_requests_per_sec: Option<u64>,
}

fn default_true() -> bool { true }

// ─────────────────────────────────────────────────────────────────────────────
// Shared state
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Clone)]
pub struct HarnessState {
    /// Registered routes keyed by path_pattern.
    routes: Arc<RwLock<HashMap<String, HarnessRoute>>>,
    /// Admin key required in `X-Admin-Key` header.
    admin_key: String,
    /// Shared reqwest client for proxying.
    http_client: reqwest::Client,
    /// Per-route rate limiters keyed by route ID.
    rate_limiters: Arc<RwLock<HashMap<String, Arc<RateLimiter>>>>,
    /// Gateway-level default request timeout.
    default_timeout_ms: Option<u64>,
}

impl HarnessState {
    fn new(admin_key: impl Into<String>, default_timeout_ms: Option<u64>) -> Self {
        Self {
            routes: Arc::new(RwLock::new(HashMap::new())),
            admin_key: admin_key.into(),
            http_client: reqwest::Client::new(),
            rate_limiters: Arc::new(RwLock::new(HashMap::new())),
            default_timeout_ms,
        }
    }

    async fn register(&self, route: HarnessRoute) -> bool {
        let mut routes = self.routes.write().await;
        if routes.contains_key(&route.path_pattern) {
            return false;
        }
        // Create rate limiter if configured.
        if let Some(rps) = route.max_requests_per_sec {
            let limiter = Arc::new(RateLimiter::new(rps, Duration::from_secs(1)));
            self.rate_limiters
                .write()
                .await
                .insert(route.id.clone(), limiter);
        }
        routes.insert(route.path_pattern.clone(), route);
        true
    }

    async fn deregister(&self, route_id: &str) -> bool {
        let mut routes = self.routes.write().await;
        let found = routes.values().any(|r| r.id == route_id);
        if found {
            routes.retain(|_, r| r.id != route_id);
            self.rate_limiters.write().await.remove(route_id);
        }
        found
    }

    async fn list(&self) -> Vec<HarnessRoute> {
        self.routes.read().await.values().cloned().collect()
    }

    async fn lookup(&self, path: &str) -> Option<HarnessRoute> {
        let routes = self.routes.read().await;
        routes.get(path).cloned()
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Admin handlers
// ─────────────────────────────────────────────────────────────────────────────

fn check_auth(
    headers: &HeaderMap,
    state: &HarnessState,
) -> Result<(), (StatusCode, Json<Value>)> {
    let key = headers
        .get("x-admin-key")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    if key == state.admin_key {
        Ok(())
    } else {
        Err((
            StatusCode::UNAUTHORIZED,
            Json(json!({ "error": "unauthorized" })),
        ))
    }
}

async fn admin_list_routes(
    State(state): State<HarnessState>,
    headers: HeaderMap,
) -> impl IntoResponse {
    if let Err(e) = check_auth(&headers, &state) {
        return e.into_response();
    }
    (StatusCode::OK, Json(state.list().await)).into_response()
}

async fn admin_register_route(
    State(state): State<HarnessState>,
    headers: HeaderMap,
    Json(body): Json<RegisterRouteBody>,
) -> impl IntoResponse {
    if let Err(e) = check_auth(&headers, &state) {
        return e.into_response();
    }
    let route = HarnessRoute {
        id: body.id.clone(),
        path_pattern: body.path_pattern,
        backend_url: body.backend_url,
        method: body.method,
        enabled: body.enabled,
        timeout_ms: body.timeout_ms,
        max_requests_per_sec: body.max_requests_per_sec,
    };
    if state.register(route).await {
        (StatusCode::CREATED, Json(json!({ "registered": body.id }))).into_response()
    } else {
        (
            StatusCode::CONFLICT,
            Json(json!({ "error": "route already exists" })),
        )
            .into_response()
    }
}

async fn admin_deregister_route(
    State(state): State<HarnessState>,
    headers: HeaderMap,
    Path(route_id): Path<String>,
) -> impl IntoResponse {
    if let Err(e) = check_auth(&headers, &state) {
        return e.into_response();
    }
    if state.deregister(&route_id).await {
        (StatusCode::OK, Json(json!({ "deregistered": route_id }))).into_response()
    } else {
        (
            StatusCode::NOT_FOUND,
            Json(json!({ "error": "route not found" })),
        )
            .into_response()
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Proxy handler
// ─────────────────────────────────────────────────────────────────────────────

async fn proxy_handler(
    State(state): State<HarnessState>,
    req: Request<Body>,
) -> impl IntoResponse {
    let path = req.uri().path().to_string();

    // Look up route.
    let route = match state.lookup(&path).await {
        Some(r) if r.enabled => r,
        Some(_) => {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(json!({ "error": "route disabled" })),
            )
                .into_response();
        }
        None => {
            return (
                StatusCode::NOT_FOUND,
                Json(json!({ "error": "no route registered for this path" })),
            )
                .into_response();
        }
    };

    // Rate limit check.
    let limiters = state.rate_limiters.read().await;
    if let Some(limiter) = limiters.get(&route.id) {
        let client_key = req
            .headers()
            .get("x-client-id")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("default");
        if !limiter.check(client_key) {
            // Compute retry-after = 1 second (fixed window).
            return (
                StatusCode::TOO_MANY_REQUESTS,
                [("Retry-After", "1")],
                Json(json!({ "error": "rate_limit_exceeded" })),
            )
                .into_response();
        }
    }
    drop(limiters);

    // Effective timeout: per-route overrides gateway default.
    let timeout_ms = route.timeout_ms.or(state.default_timeout_ms);

    // Build backend URL.
    let backend_url = format!("{}{}", route.backend_url, path);

    // Dispatch with optional timeout.
    let dispatch = state.http_client.get(&backend_url).send();

    let result = match timeout_ms {
        Some(ms) => {
            match tokio::time::timeout(Duration::from_millis(ms), dispatch).await {
                Ok(r) => r,
                Err(_) => {
                    return (
                        StatusCode::GATEWAY_TIMEOUT,
                        Json(json!({
                            "error": "deadline_exceeded",
                            "route_id": route.id,
                            "timeout_ms": ms,
                        })),
                    )
                        .into_response();
                }
            }
        }
        None => dispatch.await,
    };

    match result {
        Ok(resp) => {
            let status = StatusCode::from_u16(resp.status().as_u16())
                .unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
            let body: Value = resp
                .json()
                .await
                .unwrap_or(json!({ "error": "invalid_backend_response" }));
            (status, Json(body)).into_response()
        }
        Err(e) => (
            StatusCode::BAD_GATEWAY,
            Json(json!({ "error": format!("backend_error: {e}") })),
        )
            .into_response(),
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// GatewayTestHarness
// ─────────────────────────────────────────────────────────────────────────────

/// Builder for [`GatewayTestHarness`].
pub struct HarnessBuilder {
    admin_key: String,
    default_timeout_ms: Option<u64>,
}

impl Default for HarnessBuilder {
    fn default() -> Self {
        Self {
            admin_key: "test-admin-key".to_string(),
            default_timeout_ms: None,
        }
    }
}

impl HarnessBuilder {
    pub fn admin_key(mut self, key: impl Into<String>) -> Self {
        self.admin_key = key.into();
        self
    }

    pub fn default_timeout_ms(mut self, ms: u64) -> Self {
        self.default_timeout_ms = Some(ms);
        self
    }

    pub async fn build(self) -> GatewayTestHarness {
        GatewayTestHarness::start(self.admin_key, self.default_timeout_ms).await
    }
}

/// An in-process gateway with auth, rate limiting, deadline enforcement,
/// and HTTP proxy dispatch — all running on a random port.
pub struct GatewayTestHarness {
    /// Bound address of the gateway server.
    pub addr: SocketAddr,
    /// Pre-configured HTTP client pointing at the gateway.
    pub client: reqwest::Client,
    /// Admin key to include in admin requests.
    pub admin_key: String,
    shutdown_tx: Option<oneshot::Sender<()>>,
}

impl GatewayTestHarness {
    async fn start(admin_key: impl Into<String>, default_timeout_ms: Option<u64>) -> Self {
        let admin_key = admin_key.into();
        let state = HarnessState::new(&admin_key, default_timeout_ms);

        let app = Router::new()
            .route("/admin/routes", get(admin_list_routes).post(admin_register_route))
            .route("/admin/routes/:id", delete(admin_deregister_route))
            .fallback(any(proxy_handler))
            .with_state(state);

        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        let (shutdown_tx, shutdown_rx) = oneshot::channel::<()>();

        tokio::spawn(async move {
            axum::serve(listener, app)
                .with_graceful_shutdown(async {
                    shutdown_rx.await.ok();
                })
                .await
                .ok();
        });

        let base = format!("http://{}", addr);
        let client = reqwest::Client::builder()
            .build()
            .expect("reqwest client build failed");

        // Small yield to let the server bind.
        tokio::time::sleep(Duration::from_millis(10)).await;

        Self {
            addr,
            client,
            admin_key,
            shutdown_tx: Some(shutdown_tx),
        }
    }

    /// Gateway base URL, e.g. `"http://127.0.0.1:PORT"`.
    pub fn url(&self) -> String {
        format!("http://{}", self.addr)
    }

    /// Register a route via the admin API.
    pub async fn register_route(&self, body: serde_json::Value) -> reqwest::Response {
        self.client
            .post(format!("{}/admin/routes", self.url()))
            .header("x-admin-key", &self.admin_key)
            .json(&body)
            .send()
            .await
            .expect("register_route request failed")
    }

    /// Deregister a route via the admin API.
    pub async fn deregister_route(&self, route_id: &str) -> reqwest::Response {
        self.client
            .delete(format!("{}/admin/routes/{}", self.url(), route_id))
            .header("x-admin-key", &self.admin_key)
            .send()
            .await
            .expect("deregister_route request failed")
    }

    /// List routes via the admin API.
    pub async fn list_routes(&self) -> reqwest::Response {
        self.client
            .get(format!("{}/admin/routes", self.url()))
            .header("x-admin-key", &self.admin_key)
            .send()
            .await
            .expect("list_routes request failed")
    }
}

impl Drop for GatewayTestHarness {
    fn drop(&mut self) {
        if let Some(tx) = self.shutdown_tx.take() {
            let _ = tx.send(());
        }
    }
}
