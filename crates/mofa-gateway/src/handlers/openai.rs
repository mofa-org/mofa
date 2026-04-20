//! OpenAI-compatible API endpoints
//!
//! This module provides OpenAI-compatible chat completion endpoints
//! that route requests through [`InvocationTarget`] dispatch:
//!
//! 1. **Resolve** — check the [`AgentRegistry`] for a matching agent.
//! 2. **Dispatch** — route to the registered agent OR fall back to the
//!    [`InferenceOrchestrator`] when no agent is found.
//!
//! This replaces the previous mock response ("Hello from MoFA gateway!")
//! with real routing logic aligned with the Gateway V2 design.

use axum::{
    extract::Extension,
    http::HeaderMap,
    response::IntoResponse,
    routing::post,
    Json, Router,
};
use std::sync::Arc;

use crate::error::GatewayError;
use crate::handlers::InvocationRouter;
use crate::inference_bridge::ChatCompletionRequest;

/// Create the OpenAI-compatible router.
///
/// The router does not use axum `State` — all dependencies are injected
/// via [`axum::Extension`] so the router can be merged into any parent
/// without requiring a concrete state type.
pub fn openai_router() -> Router {
    Router::new()
        .route("/v1/chat/completions", post(chat_completions))
}

/// Extract an opaque client key from request headers for logging/rate-limiting.
///
/// Uses `X-Forwarded-For` when available, otherwise falls back to `"unknown"`.
fn client_key(headers: &HeaderMap) -> String {
    headers
        .get("x-forwarded-for")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.split(',').next().unwrap_or(s).trim().to_string())
        .unwrap_or_else(|| "unknown".to_string())
}

/// `POST /v1/chat/completions`
///
/// OpenAI-compatible chat completion endpoint with [`InvocationTarget`] routing.
///
/// # Routing
///
/// 1. The `model` field in the request body is used as the agent lookup key.
/// 2. If the [`AgentRegistry`] contains an entry for that key, the request is
///    dispatched as [`InvocationTarget::Agent`] — the agent's metadata is
///    returned until full agent invocation is implemented in a follow-up PR.
/// 3. If no agent matches, the request falls through to the
///    [`InferenceOrchestrator`] as [`InvocationTarget::LocalInference`].
pub async fn chat_completions(
    Extension(router): Extension<Arc<InvocationRouter>>,
    headers: HeaderMap,
    Json(req): Json<ChatCompletionRequest>,
) -> Result<impl IntoResponse, GatewayError> {
    let client = client_key(&headers);
    tracing::info!(
        target = "mofa_gateway::openai",
        client = %client,
        model = %req.model,
        "POST /v1/chat/completions"
    );

    // Step 1: resolve InvocationTarget from model name + registry
    let target = router.resolve(&req.model).await;
    tracing::debug!(
        target = "mofa_gateway::openai",
        invocation_target = %target,
        "InvocationTarget resolved"
    );

    // Step 2: dispatch and return
    let response = router.dispatch(target, req).await?;
    Ok(Json(response))
}

#[cfg(test)]
mod tests {
    use super::*;
    use mofa_foundation::inference::OrchestratorConfig;
    use mofa_runtime::agent::registry::AgentRegistry;
    use crate::inference_bridge::{InferenceBridge, Message};

    fn make_router_ext() -> Extension<Arc<InvocationRouter>> {
        let registry = Arc::new(AgentRegistry::new());
        let bridge = Arc::new(InferenceBridge::new(OrchestratorConfig::default()));
        let router = Arc::new(InvocationRouter::new(registry, bridge));
        Extension(router)
    }

    fn make_request(model: &str) -> ChatCompletionRequest {
        ChatCompletionRequest {
            model: model.to_string(),
            messages: vec![Message { role: "user".to_string(), content: "Hello".to_string() }],
            max_tokens: Some(64),
            temperature: Some(0.7),
            stream: Some(false),
        }
    }

    #[tokio::test]
    async fn test_chat_completions_unknown_model_falls_back_to_inference() {
        let router_ext = make_router_ext();
        // No agent registered — should fall through to LocalInference
        let result = chat_completions(
            router_ext,
            HeaderMap::new(),
            Json(make_request("gpt-4")),
        )
        .await;
        assert!(result.is_ok());
    }
}
