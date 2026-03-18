//! OpenAI-compatible API endpoints
//!
//! This module provides OpenAI-compatible chat completion endpoints
//! that bridge to the InferenceOrchestrator.

use axum::{
    Extension, Json, Router,
    extract::State,
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    routing::post,
};
use std::sync::Arc;

use crate::error::GatewayError;
use crate::inference_bridge::{ChatCompletionRequest, ChatCompletionResponse, InferenceBridge};

/// Create the OpenAI-compatible router
pub fn openai_router() -> Router {
    Router::new().route("/v1/chat/completions", post(chat_completions))
}

/// Extract client key from request headers for rate-limiting
fn client_key(headers: &HeaderMap) -> String {
    headers
        .get("x-forwarded-for")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.split(',').next().unwrap_or(s).trim().to_string())
        .unwrap_or_else(|| "unknown".to_string())
}

/// POST /v1/chat/completions
///
/// OpenAI-compatible chat completion endpoint.
/// Routes to InferenceOrchestrator for actual inference.
pub async fn chat_completions(
    Extension(bridge): Extension<Arc<InferenceBridge>>,
    headers: HeaderMap,
    Json(req): Json<ChatCompletionRequest>,
) -> Result<impl IntoResponse, GatewayError> {
    // Rate-limit check - simplified for now
    let client = client_key(&headers);

    // Demo logging
    println!("Routing request via InferenceOrchestrator");
    println!("  Model: {}", req.model);

    // Run chat completion via bridge
    let response = bridge.run_chat_completion(req).await?;

    Ok(Json(response))
}

#[cfg(test)]
mod tests {
    use super::*;
    use mofa_foundation::inference::OrchestratorConfig;

    #[tokio::test]
    async fn test_chat_completions_endpoint() {
        // Create bridge with default config
        let config = OrchestratorConfig::default();
        let bridge = InferenceBridge::new(config);

        // Create a request
        let request = ChatCompletionRequest {
            model: "gpt-4".to_string(),
            messages: vec![crate::inference_bridge::Message {
                role: "user".to_string(),
                content: "Hello".to_string(),
            }],
            max_tokens: Some(100),
            temperature: Some(0.7),
            stream: Some(false),
        };

        // Run completion
        let response = bridge.run_chat_completion(request).await.unwrap();

        // Verify response format
        assert!(response.id.starts_with("chatcmpl-"));
        assert_eq!(response.object_type, "chat.completion");
        assert_eq!(response.choices.len(), 1);
    }
}
