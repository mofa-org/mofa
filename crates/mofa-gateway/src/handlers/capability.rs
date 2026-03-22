//! Gateway capability endpoints.
//!
//! POST /capability/invoke - invoke a registered capability
//! GET  /capability/list   - list registered capability names

use axum::{
    Json, Router,
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{get, post},
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use uuid::Uuid;

use mofa_foundation::{CapabilityRequest, CapabilityResponse};

use crate::state::AppState;

/// Build the capability API router.
pub fn capability_router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/capability/invoke", post(invoke_capability))
        .route("/capability/list", get(list_capabilities))
}

/// JSON payload for `POST /capability/invoke`.
#[derive(Debug, Deserialize)]
pub struct InvokeCapabilityRequest {
    /// Registered capability name to invoke.
    pub capability: String,
    /// Primary textual input for the capability.
    pub input: String,
    /// Arbitrary structured arguments forwarded to the capability.
    #[serde(default)]
    pub params: HashMap<String, Value>,
    /// Optional caller-supplied trace identifier.
    pub trace_id: Option<String>,
}

#[derive(Debug, Serialize)]
struct CapabilityListResponse {
    capabilities: Vec<String>,
    trace_id: String,
}

#[derive(Debug, Serialize)]
struct ErrorResponse {
    error: String,
    trace_id: String,
}

fn trace_id() -> String {
    Uuid::new_v4().to_string()
}

fn registry_or_503(
    state: &AppState,
) -> Result<Arc<mofa_foundation::GatewayCapabilityRegistry>, (StatusCode, Json<ErrorResponse>)> {
    match &state.capability_registry {
        Some(registry) => Ok(Arc::clone(registry)),
        None => Err((
            StatusCode::SERVICE_UNAVAILABLE,
            Json(ErrorResponse {
                error: "capability registry not configured".to_string(),
                trace_id: trace_id(),
            }),
        )),
    }
}

/// List the names of all registered capabilities.
pub async fn list_capabilities(State(state): State<Arc<AppState>>) -> Response {
    let registry = match registry_or_503(&state) {
        Ok(registry) => registry,
        Err(resp) => return resp.into_response(),
    };

    (
        StatusCode::OK,
        Json(CapabilityListResponse {
            capabilities: registry.names(),
            trace_id: trace_id(),
        }),
    )
        .into_response()
}

/// Invoke a registered capability with the provided request payload.
pub async fn invoke_capability(
    State(state): State<Arc<AppState>>,
    Json(req): Json<InvokeCapabilityRequest>,
) -> Response {
    let registry = match registry_or_503(&state) {
        Ok(registry) => registry,
        Err(resp) => return resp.into_response(),
    };

    let capability_name = req.capability.trim();
    if capability_name.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "capability must not be empty".to_string(),
                trace_id: trace_id(),
            }),
        )
            .into_response();
    }

    let capability = match registry.get(capability_name) {
        Some(capability) => capability,
        None => {
            return (
                StatusCode::NOT_FOUND,
                Json(ErrorResponse {
                    error: format!("unknown capability '{capability_name}'"),
                    trace_id: trace_id(),
                }),
            )
                .into_response();
        }
    };

    let capability_trace_id = req.trace_id.unwrap_or_else(trace_id);
    match capability
        .invoke(CapabilityRequest {
            input: req.input,
            params: req.params,
            trace_id: capability_trace_id.clone(),
        })
        .await
    {
        Ok(response) => (StatusCode::OK, Json(response)).into_response(),
        Err(error) => (
            StatusCode::BAD_GATEWAY,
            Json(ErrorResponse {
                error: error.to_string(),
                trace_id: capability_trace_id,
            }),
        )
            .into_response(),
    }
}
