//! HTTP handlers for mofa-local-llm proxy endpoints.

use crate::error::{GatewayError, GatewayResult};
use crate::gateway::{CircuitBreaker, GatewayState};
use crate::types::NodeStatus;
use axum::body::Body;
use axum::extract::{Path, Request, State};
use axum::response::IntoResponse;
use axum::response::Response;
use std::sync::Arc;
use tracing::info;

/// Check health and circuit breaker before proxying.
async fn check_backend_health(state: &GatewayState) -> GatewayResult<()> {
    let node_id = state
        .local_llm_node_id
        .as_ref()
        .ok_or_else(|| GatewayError::Network("Local LLM proxy not enabled".to_string()))?;

    // Check circuit breaker
    let breaker: Arc<CircuitBreaker> = state.circuit_breakers.get_or_create(node_id).await;
    if !breaker.try_acquire().await? {
        return Err(GatewayError::CircuitBreakerOpen(node_id.to_string()));
    }

    // Check health status
    let health_status = state.health_checker.get_status(node_id).await;
    if let Some(status) = health_status {
        if status != NodeStatus::Healthy {
            breaker.record_failure().await;
            return Err(GatewayError::UnhealthyNode(node_id.to_string()));
        }
    } else {
        // If no status, perform a quick health check
        let is_healthy = state.health_checker.check_node(node_id).await?;
        if !is_healthy {
            breaker.record_failure().await;
            return Err(GatewayError::UnhealthyNode(node_id.to_string()));
        }
    }

    Ok(())
}

/// Forward a request to the local-LLM backend and record circuit-breaker outcome.
async fn proxy_and_record(
    state: &GatewayState,
    request: Request<Body>,
    path: &str,
) -> Result<Response<Body>, GatewayError> {
    let proxy_handler = state
        .local_llm_proxy
        .as_ref()
        .ok_or_else(|| GatewayError::Network("Local LLM proxy not enabled".to_string()))?;

    let node_id = state
        .local_llm_node_id
        .as_ref()
        .ok_or_else(|| GatewayError::Network("Local LLM proxy not enabled".to_string()))?;

    let result = proxy_handler.forward(request, path).await;

    let breaker: Arc<CircuitBreaker> = state.circuit_breakers.get_or_create(node_id).await;
    match result {
        Ok(response) => {
            breaker.record_success().await;
            Ok(response)
        }
        Err(e) => {
            breaker.record_failure().await;
            Err(e)
        }
    }
}

/// Proxy chat completions request to mofa-local-llm.
pub async fn proxy_local_llm_chat(
    State(state): State<GatewayState>,
    request: Request<Body>,
) -> Result<impl IntoResponse, GatewayError> {
    info!("Proxying chat completions request to mofa-local-llm");
    check_backend_health(&state).await?;
    Ok(proxy_and_record(&state, request, "v1/chat/completions").await?)
}

/// Proxy models list request to mofa-local-llm.
pub async fn proxy_local_llm_models(
    State(state): State<GatewayState>,
    request: Request<Body>,
) -> Result<impl IntoResponse, GatewayError> {
    info!("Proxying models list request to mofa-local-llm");
    check_backend_health(&state).await?;
    Ok(proxy_and_record(&state, request, "v1/models").await?)
}

/// Proxy model info request to mofa-local-llm.
pub async fn proxy_local_llm_model_info(
    State(state): State<GatewayState>,
    Path(model_id): Path<String>,
    request: Request<Body>,
) -> Result<impl IntoResponse, GatewayError> {
    info!(model_id = %model_id, "Proxying model info request to mofa-local-llm");
    check_backend_health(&state).await?;
    let path = format!("v1/models/{}", model_id);
    Ok(proxy_and_record(&state, request, &path).await?)
}
