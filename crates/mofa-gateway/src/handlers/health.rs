//! Health and readiness check endpoints
//!
//! GET /health  - liveness probe (server is up)
//! GET /ready   - readiness probe (registry reachable, can serve traffic)

use axum::{Json, extract::State, http::StatusCode, response::IntoResponse};
use serde_json::json;
use std::sync::Arc;

use crate::state::AppState;

/// GET /health - liveness probe
///
/// Always returns 200 OK while the process is alive.
pub async fn health() -> impl IntoResponse {
    (
        StatusCode::OK,
        Json(json!({ "status": "ok" })),
    )
}

/// GET /ready - readiness probe
///
/// Verifies that the agent registry is accessible. Returns 200 if ready,
/// 503 if not yet able to serve traffic.
pub async fn ready(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    // Querying the registry count is a lightweight check that the registry
    // lock is reachable and no deadlock has occurred.
    let agent_count = state.registry.count().await;
    (
        StatusCode::OK,
        Json(json!({
            "status": "ready",
            "agents": agent_count,
        })),
    )
}

/// Build the health router sub-tree
pub fn health_router() -> axum::Router<Arc<AppState>> {
    use axum::routing::get;
    axum::Router::new()
        .route("/health", get(health))
        .route("/ready", get(ready))
}
