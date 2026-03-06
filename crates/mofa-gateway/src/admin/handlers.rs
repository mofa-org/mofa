//! Admin API request handlers.

use std::sync::Arc;

use axum::{
    Json,
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    routing::{delete, get, patch, post},
    Router,
};
use serde_json::json;

use mofa_kernel::gateway::route::GatewayRoute;

use super::state::{AdminState, parse_method};
use super::types::{AdminRouteEntry, GatewayHealthSummary, RegisterRouteRequest};

// ─────────────────────────────────────────────────────────────────────────────
// Auth guard
// ─────────────────────────────────────────────────────────────────────────────

/// Extract and verify the `X-Admin-Key` header.  Returns `Err(401)` on
/// missing or invalid credentials.
fn require_auth(
    headers: &HeaderMap,
    state: &AdminState,
) -> Result<(), (StatusCode, axum::Json<serde_json::Value>)> {
    let key = headers
        .get("x-admin-key")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");

    if state.verify_key(key) {
        Ok(())
    } else {
        Err((
            StatusCode::UNAUTHORIZED,
            Json(json!({ "error": { "code": "UNAUTHORIZED", "message": "invalid or missing X-Admin-Key" } })),
        ))
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// GET /admin/routes
// ─────────────────────────────────────────────────────────────────────────────

/// List all registered routes with per-route request counts.
pub async fn list_routes(
    State(state): State<Arc<AdminState>>,
    headers: HeaderMap,
) -> impl IntoResponse {
    if let Err(e) = require_auth(&headers, &state) {
        return e.into_response();
    }

    let entries: Vec<AdminRouteEntry> = state
        .list_routes()
        .into_iter()
        .map(|(r, count)| AdminRouteEntry {
            id: r.id,
            path_pattern: r.path_pattern,
            agent_id: r.agent_id,
            method: r.method.to_string(),
            priority: r.priority,
            enabled: r.enabled,
            requests_served: count,
        })
        .collect();

    (StatusCode::OK, Json(entries)).into_response()
}

// ─────────────────────────────────────────────────────────────────────────────
// POST /admin/routes
// ─────────────────────────────────────────────────────────────────────────────

/// Register a new route at runtime.
///
/// Returns 201 on success, 409 if the route ID already exists, 422 if the
/// request body is invalid.
pub async fn register_route(
    State(state): State<Arc<AdminState>>,
    headers: HeaderMap,
    Json(req): Json<RegisterRouteRequest>,
) -> impl IntoResponse {
    if let Err(e) = require_auth(&headers, &state) {
        return e.into_response();
    }

    if req.id.trim().is_empty() {
        return (
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(json!({ "error": { "code": "INVALID_ROUTE", "message": "route id cannot be empty" } })),
        )
            .into_response();
    }

    if req.path_pattern.trim().is_empty() || !req.path_pattern.starts_with('/') {
        return (
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(json!({ "error": { "code": "INVALID_ROUTE", "message": "path_pattern must be non-empty and start with '/'" } })),
        )
            .into_response();
    }

    if req.agent_id.trim().is_empty() {
        return (
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(json!({ "error": { "code": "INVALID_ROUTE", "message": "agent_id cannot be empty" } })),
        )
            .into_response();
    }

    let method = parse_method(&req.method);
    let mut route = GatewayRoute::new(&req.id, &req.agent_id, &req.path_pattern, method);
    if !req.enabled {
        route = route.disabled();
    }

    if !state.register_route(route) {
        return (
            StatusCode::CONFLICT,
            Json(json!({ "error": { "code": "ROUTE_ALREADY_EXISTS", "message": format!("route '{}' is already registered", req.id) } })),
        )
            .into_response();
    }

    (
        StatusCode::CREATED,
        Json(json!({ "registered": req.id })),
    )
        .into_response()
}

// ─────────────────────────────────────────────────────────────────────────────
// DELETE /admin/routes/:id
// ─────────────────────────────────────────────────────────────────────────────

/// Deregister a route by ID.
///
/// In-flight requests on the deregistered route complete normally; new
/// requests receive 404 immediately after this call returns.
pub async fn deregister_route(
    State(state): State<Arc<AdminState>>,
    headers: HeaderMap,
    Path(route_id): Path<String>,
) -> impl IntoResponse {
    if let Err(e) = require_auth(&headers, &state) {
        return e.into_response();
    }

    if state.deregister_route(&route_id) {
        (StatusCode::OK, Json(json!({ "deregistered": route_id }))).into_response()
    } else {
        (
            StatusCode::NOT_FOUND,
            Json(json!({ "error": { "code": "ROUTE_NOT_FOUND", "message": format!("route '{}' not found", route_id) } })),
        )
            .into_response()
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// PATCH /admin/routes/:id
// ─────────────────────────────────────────────────────────────────────────────

/// Toggle a route's `enabled` flag at runtime without deregistering it.
///
/// Accepts `{"enabled": true}` or `{"enabled": false}`.
/// Returns 200 on success, 404 if the route does not exist.
pub async fn toggle_route(
    State(state): State<Arc<AdminState>>,
    headers: HeaderMap,
    Path(route_id): Path<String>,
    Json(body): Json<serde_json::Value>,
) -> impl IntoResponse {
    if let Err(e) = require_auth(&headers, &state) {
        return e.into_response();
    }

    let enabled = match body.get("enabled").and_then(|v| v.as_bool()) {
        Some(v) => v,
        None => {
            return (
                StatusCode::UNPROCESSABLE_ENTITY,
                Json(json!({ "error": { "code": "INVALID_BODY", "message": "body must be {\"enabled\": true|false}" } })),
            )
                .into_response();
        }
    };

    if state.set_route_enabled(&route_id, enabled) {
        (StatusCode::OK, Json(json!({ "id": route_id, "enabled": enabled }))).into_response()
    } else {
        (
            StatusCode::NOT_FOUND,
            Json(json!({ "error": { "code": "ROUTE_NOT_FOUND", "message": format!("route '{}' not found", route_id) } })),
        )
            .into_response()
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// GET /admin/health
// ─────────────────────────────────────────────────────────────────────────────

/// Return a health summary of the gateway.
pub async fn admin_health(
    State(state): State<Arc<AdminState>>,
    headers: HeaderMap,
) -> impl IntoResponse {
    if let Err(e) = require_auth(&headers, &state) {
        return e.into_response();
    }

    let summary = GatewayHealthSummary {
        uptime_secs: state.uptime_secs(),
        total_requests: state.total_requests(),
        routes_count: state.routes_count(),
        hot_reload_active: state.hot_reload_active(),
        version: env!("CARGO_PKG_VERSION").to_string(),
    };

    (StatusCode::OK, Json(summary)).into_response()
}

// ─────────────────────────────────────────────────────────────────────────────
// Router
// ─────────────────────────────────────────────────────────────────────────────

/// Build the admin router subtree.
pub fn admin_router() -> Router<Arc<AdminState>> {
    Router::new()
        .route("/admin/routes", get(list_routes).post(register_route))
        .route(
            "/admin/routes/:id",
            delete(deregister_route).patch(toggle_route),
        )
        .route("/admin/health", get(admin_health))
}
