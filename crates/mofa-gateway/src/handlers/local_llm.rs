//! HTTP handlers for mofa-local-llm proxy endpoints.

use crate::error::{GatewayError, GatewayResult};
use crate::gateway::{CircuitBreaker, GatewayState};
use crate::types::NodeStatus;
use axum::body::Body;
use axum::extract::{Path, State};
use axum::http::{Request, StatusCode};
use axum::response::{IntoResponse, Response};
use std::sync::Arc;
use tracing::{info, warn};

/// Check health and circuit breaker before proxying.
async fn check_backend_health(
    state: &GatewayState,
) -> GatewayResult<()> {
    let node_id = state
        .local_llm_node_id
        .as_ref()
        .ok_or_else(|| GatewayError::Network("Local LLM proxy not enabled".to_string()))?;

    // Check circuit breaker
    let breaker: Arc<crate::gateway::CircuitBreaker> = state.circuit_breakers.get_or_create(node_id).await;
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

/// Proxy chat completions request to mofa-local-llm.
pub async fn proxy_local_llm_chat(
    State(state): State<GatewayState>,
    request: Request<Body>,
) -> Result<impl IntoResponse, GatewayError> {
    info!("Proxying chat completions request to mofa-local-llm");

    // Check health and circuit breaker
    check_backend_health(&state).await?;

    let proxy_handler = state
        .local_llm_proxy
        .as_ref()
        .ok_or_else(|| GatewayError::Network("Local LLM proxy not enabled".to_string()))?;

    let node_id = state
        .local_llm_node_id
        .as_ref()
        .ok_or_else(|| GatewayError::Network("Local LLM proxy not enabled".to_string()))?;

    // Forward request to backend
    let result = proxy_handler.forward(request, "v1/chat/completions").await;

    match result {
        Ok(response) => {
            // Record success
            let breaker: Arc<crate::gateway::CircuitBreaker> = state.circuit_breakers.get_or_create(node_id).await;
            breaker.record_success().await;
            // Response<Body> already implements IntoResponse, return directly
            Ok(response)
        }
        Err(e) => {
            // Record failure
            let breaker: Arc<crate::gateway::CircuitBreaker> = state.circuit_breakers.get_or_create(node_id).await;
            breaker.record_failure().await;
            Err(e)
        }
    }
}

/// Proxy models list request to mofa-local-llm.
pub async fn proxy_local_llm_models(
    State(state): State<GatewayState>,
    request: Request<Body>,
) -> Result<impl IntoResponse, GatewayError> {
    info!("Proxying models list request to mofa-local-llm");

    // Check health and circuit breaker
    check_backend_health(&state).await?;

    let proxy_handler = state
        .local_llm_proxy
        .as_ref()
        .ok_or_else(|| GatewayError::Network("Local LLM proxy not enabled".to_string()))?;

    let node_id = state
        .local_llm_node_id
        .as_ref()
        .ok_or_else(|| GatewayError::Network("Local LLM proxy not enabled".to_string()))?;

    // Forward request to backend
    let result = proxy_handler.forward(request, "v1/models").await;

    match result {
        Ok(response) => {
            // Record success
            let breaker: Arc<crate::gateway::CircuitBreaker> = state.circuit_breakers.get_or_create(node_id).await;
            breaker.record_success().await;
            // Response<Body> already implements IntoResponse, return directly
            Ok(response)
        }
        Err(e) => {
            // Record failure
            let breaker: Arc<crate::gateway::CircuitBreaker> = state.circuit_breakers.get_or_create(node_id).await;
            breaker.record_failure().await;
            Err(e)
        }
    }
}

/// Proxy model info request to mofa-local-llm.
pub async fn proxy_local_llm_model_info(
    State(state): State<GatewayState>,
    Path(model_id): Path<String>,
    request: Request<Body>,
) -> Result<impl IntoResponse, GatewayError> {
    tracing::info!(model_id = %model_id, path = %request.uri().path(), "Model info request received");
    info!(model_id = %model_id, "Proxying model info request to mofa-local-llm");

    // Check health and circuit breaker
    check_backend_health(&state).await?;

    let proxy_handler = state
        .local_llm_proxy
        .as_ref()
        .ok_or_else(|| GatewayError::Network("Local LLM proxy not enabled".to_string()))?;

    let node_id = state
        .local_llm_node_id
        .as_ref()
        .ok_or_else(|| GatewayError::Network("Local LLM proxy not enabled".to_string()))?;

    // Forward request to backend
    let path = format!("v1/models/{}", model_id);
    tracing::debug!(path = %path, "Forwarding to backend");
    let result = proxy_handler.forward(request, &path).await;

    match result {
        Ok(response) => {
            tracing::debug!(status = %response.status(), "Backend response received");
            // Record success
            let breaker: Arc<crate::gateway::CircuitBreaker> = state.circuit_breakers.get_or_create(node_id).await;
            breaker.record_success().await;
            // Response<Body> already implements IntoResponse, return directly
            Ok(response)
        }
        Err(e) => {
            warn!(error = %e, "Failed to proxy model info request");
            // Record failure
            let breaker: Arc<crate::gateway::CircuitBreaker> = state.circuit_breakers.get_or_create(node_id).await;
            breaker.record_failure().await;
            Err(e)
        }
    }
}
