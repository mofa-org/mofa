//! Axum-based HTTP gateway server.
//!
//! [`GatewayServer`] wires together the router, filter pipeline, and backend
//! registry into a running axum service.
//!
//! # Endpoints
//!
//! | Method | Path | Description |
//! |--------|------|-------------|
//! | `GET`  | `/health` | Liveness check — always `200 OK`. |
//! | `ANY`  | `/v1/chat/completions` | Proxy to the registered OpenAI backend. |
//! | `GET`  | `/v1/capabilities` | List all registered backends as JSON. |

use crate::backend::{InMemoryCapabilityRegistry, OpenAiBackend};
use crate::filter::{ApiKeyFilter, FilterPipeline, LoggingFilter, RateLimitFilter};
use crate::router::TrieRouter;
use axum::{
    Json, Router,
    body::Bytes,
    extract::State,
    http::{HeaderMap, Method, StatusCode, Uri},
    response::{IntoResponse, Response},
    routing::{any, get},
};
use mofa_kernel::gateway::{
    BackendKind, CapabilityDescriptor, CapabilityRegistry, FilterAction, GatewayConfig,
    GatewayContext, GatewayRequest, GatewayResponse, GatewayRouter, HttpMethod, RouteConfig,
};
use serde_json::json;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::info;
use uuid::Uuid;

// ─────────────────────────────────────────────────────────────────────────────
// Shared application state
// ─────────────────────────────────────────────────────────────────────────────

/// Shared state injected into every axum handler via [`State`] extractor.
#[derive(Clone)]
pub struct AppState {
    router: Arc<RwLock<TrieRouter>>,
    registry: Arc<RwLock<InMemoryCapabilityRegistry>>,
    pipeline: Arc<FilterPipeline>,
    openai_backend: Arc<OpenAiBackend>,
}

// ─────────────────────────────────────────────────────────────────────────────
// GatewayServerConfig
// ─────────────────────────────────────────────────────────────────────────────

/// Runtime configuration for [`GatewayServer`].
pub struct GatewayServerConfig {
    /// TCP port to listen on (default: 3000).
    pub port: u16,
    /// List of valid API keys for the built-in `ApiKeyFilter`.
    /// When empty, authentication is **disabled** — use only in development.
    pub api_keys: Vec<String>,
    /// Optional OpenAI API key to inject into upstream requests.
    pub openai_api_key: Option<String>,
    /// OpenAI-compatible base URL (default: `https://api.openai.com`).
    pub openai_base_url: String,
    /// Sustained rate limit (requests/second, default: 100).
    pub rate_per_second: u32,
    /// Burst capacity (default: 200).
    pub burst_capacity: u32,
}

impl Default for GatewayServerConfig {
    fn default() -> Self {
        Self {
            port: 3000,
            api_keys: Vec::new(),
            openai_api_key: None,
            openai_base_url: "https://api.openai.com".to_string(),
            rate_per_second: 100,
            burst_capacity: 200,
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// GatewayServer
// ─────────────────────────────────────────────────────────────────────────────

/// High-level gateway server encapsulating router, filter pipeline, and
/// backend registry.
pub struct GatewayServer {
    config: GatewayServerConfig,
}

impl GatewayServer {
    /// Create a new server from the given configuration.
    pub fn new(config: GatewayServerConfig) -> Self {
        Self { config }
    }

    /// Build the axum [`Router`] wired to the provided [`GatewayConfig`].
    ///
    /// This method validates the config, registers routes and backends, and
    /// constructs the filter pipeline.  Call [`start()`](Self::start) to bind
    /// and serve.
    pub fn build_app(&self, gateway_cfg: &GatewayConfig) -> Router {
        gateway_cfg.validate().expect("invalid gateway config");

        // Build the trie router from the validated route list.
        let mut trie = TrieRouter::new();
        for route in &gateway_cfg.routes {
            trie.register(route.clone())
                .expect("duplicate route in validated config");
        }

        // Build the capability registry.
        let mut registry = InMemoryCapabilityRegistry::new();
        for backend in &gateway_cfg.backends {
            registry
                .register(backend.clone())
                .expect("duplicate backend in validated config");
        }

        // Build the filter pipeline.
        let mut filters: Vec<Arc<dyn mofa_kernel::gateway::GatewayFilter>> =
            vec![Arc::new(LoggingFilter::new()), Arc::new(RateLimitFilter::new(
                self.config.rate_per_second,
                self.config.burst_capacity,
            ))];
        if !self.config.api_keys.is_empty() {
            filters.push(Arc::new(ApiKeyFilter::new(self.config.api_keys.clone())));
        }
        let pipeline = FilterPipeline::new(filters);

        // OpenAI backend.
        let openai_backend = OpenAiBackend::new(
            "openai",
            &self.config.openai_base_url,
            self.config.openai_api_key.clone(),
        );

        let state = AppState {
            router: Arc::new(RwLock::new(trie)),
            registry: Arc::new(RwLock::new(registry)),
            pipeline: Arc::new(pipeline),
            openai_backend: Arc::new(openai_backend),
        };

        Router::new()
            .route("/health", get(health_handler))
            .route("/v1/capabilities", get(list_capabilities_handler))
            .route("/v1/chat/completions", any(proxy_handler))
            .route("/v1/models", any(proxy_handler))
            .route("/v1/embeddings", any(proxy_handler))
            .with_state(state)
    }

    /// Bind the server to `0.0.0.0:{port}` and serve until the process exits.
    pub async fn start(self, gateway_cfg: GatewayConfig) -> std::io::Result<()> {
        let app = self.build_app(&gateway_cfg);
        let addr = format!("0.0.0.0:{}", self.config.port);
        info!(addr = %addr, "MoFA Cognitive Gateway starting");
        let listener = tokio::net::TcpListener::bind(&addr).await?;
        axum::serve(listener, app).await
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Handlers
// ─────────────────────────────────────────────────────────────────────────────

/// `GET /health` — liveness probe.
async fn health_handler() -> impl IntoResponse {
    Json(json!({ "status": "ok", "service": "mofa-gateway" }))
}

/// `GET /v1/capabilities` — list registered backends.
async fn list_capabilities_handler(State(state): State<AppState>) -> impl IntoResponse {
    let registry = state.registry.read().await;
    let backends: Vec<serde_json::Value> = registry
        .list_all()
        .iter()
        .map(|d| {
            json!({
                "id": d.id,
                "kind": format!("{:?}", d.kind),
                "endpoint": d.endpoint,
                "health": format!("{:?}", d.health),
            })
        })
        .collect();
    Json(json!({ "backends": backends }))
}

/// Generic proxy handler — routes request through the filter pipeline then
/// forwards to the resolved backend.
async fn proxy_handler(
    State(state): State<AppState>,
    method: Method,
    uri: Uri,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let http_method = axum_method_to_kernel(&method);
    let path = uri.path().to_string();
    let request_id = Uuid::new_v4().to_string();

    let mut req = GatewayRequest::new(&request_id, &path, http_method);
    for (name, value) in &headers {
        if let Ok(v) = value.to_str() {
            req = req.with_header(name.as_str(), v);
        }
    }
    req = req.with_body(body.to_vec());

    // Route lookup.
    let route_match = {
        let router = state.router.read().await;
        router.resolve(&path, &req.method)
    };

    let Some(route_match) = route_match else {
        return (StatusCode::NOT_FOUND, Json(json!({
            "error": format!("no route matched '{path}'")
        })))
            .into_response();
    };

    let mut ctx = GatewayContext::new(req);
    ctx.route_match = Some(route_match);

    // Run the filter pipeline on the request.
    let pipeline_result = state.pipeline.run_request(&mut ctx).await;
    match pipeline_result {
        Ok(FilterAction::Reject(status, msg)) => {
            let code = StatusCode::from_u16(status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
            return (code, Json(json!({ "error": msg }))).into_response();
        }
        Ok(FilterAction::Redirect(loc)) => {
            return (
                StatusCode::TEMPORARY_REDIRECT,
                [("location", loc)],
            )
                .into_response();
        }
        Ok(FilterAction::Continue) => {}
        // FilterAction is #[non_exhaustive]; treat unknown variants as Continue.
        Ok(_) => {}
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": e.to_string() })),
            )
                .into_response();
        }
    }

    // Forward to OpenAI-compatible backend.
    let upstream_result = state.openai_backend.forward(&ctx.request).await;

    let mut gateway_resp = match upstream_result {
        Ok(r) => r,
        Err(e) => {
            return (
                StatusCode::BAD_GATEWAY,
                Json(json!({ "error": e.to_string() })),
            )
                .into_response();
        }
    };

    // Run the filter pipeline on the response.
    let _ = state.pipeline.run_response(&ctx, &mut gateway_resp).await;

    build_axum_response(gateway_resp)
}

// ─────────────────────────────────────────────────────────────────────────────
// Helpers
// ─────────────────────────────────────────────────────────────────────────────

fn axum_method_to_kernel(m: &Method) -> HttpMethod {
    match m.as_str() {
        "GET" => HttpMethod::Get,
        "POST" => HttpMethod::Post,
        "PUT" => HttpMethod::Put,
        "PATCH" => HttpMethod::Patch,
        "DELETE" => HttpMethod::Delete,
        "HEAD" => HttpMethod::Head,
        "OPTIONS" => HttpMethod::Options,
        _ => HttpMethod::Get,
    }
}

fn build_axum_response(resp: GatewayResponse) -> Response {
    let status = StatusCode::from_u16(resp.status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
    let mut builder = axum::response::Response::builder().status(status);
    for (k, v) in &resp.headers {
        builder = builder.header(k, v);
    }
    builder.body(axum::body::Body::from(resp.body)).unwrap()
}
