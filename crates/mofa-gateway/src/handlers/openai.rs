//! OpenAI-compatible API endpoints
//!
//! This module provides OpenAI-compatible chat completion endpoints
//! that bridge to the InferenceOrchestrator.

use axum::{
    Extension, Json, Router,
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::post,
};
use serde::Serialize;
use std::sync::Arc;

use crate::inference_bridge::{ChatCompletionRequest, InferenceBridge};

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

#[derive(Debug, Serialize)]
struct OpenAiErrorBody {
    error: OpenAiErrorDetail,
}

#[derive(Debug, Serialize)]
struct OpenAiErrorDetail {
    message: String,
    #[serde(rename = "type")]
    error_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    code: Option<String>,
}

impl OpenAiErrorBody {
    fn invalid_request(message: impl Into<String>) -> Self {
        Self {
            error: OpenAiErrorDetail {
                message: message.into(),
                error_type: "invalid_request_error".to_string(),
                code: None,
            },
        }
    }
}

/// POST /v1/chat/completions
///
/// OpenAI-compatible chat completion endpoint.
/// Routes to InferenceOrchestrator for actual inference.
pub async fn chat_completions(
    Extension(bridge): Extension<Arc<InferenceBridge>>,
    headers: HeaderMap,
    Json(req): Json<ChatCompletionRequest>,
) -> Response {
    // Rate-limit check - simplified for now
    let client = client_key(&headers);

    if req.messages.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(OpenAiErrorBody::invalid_request(
                "messages must not be empty",
            )),
        )
            .into_response();
    }

    if req.stream.unwrap_or(false) {
        return (
            StatusCode::BAD_REQUEST,
            Json(OpenAiErrorBody::invalid_request(
                "stream=true is not supported on legacy /v1/chat/completions; use openai-compat SSE endpoint",
            )),
        )
            .into_response();
    }

    // Demo logging
    println!("Routing request via InferenceOrchestrator");
    println!("  Model: {}", req.model);

    // Run chat completion via bridge
    let response = match bridge.run_chat_completion(req).await {
        Ok(response) => response,
        Err(error) => return error.into_response(),
    };

    Json(response).into_response()
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

    #[tokio::test]
    async fn test_chat_completions_rejects_stream_true_with_structured_400() {
        use axum::body::to_bytes;

        let config = OrchestratorConfig::default();
        let bridge = Arc::new(InferenceBridge::new(config));

        let request = ChatCompletionRequest {
            model: "gpt-4".to_string(),
            messages: vec![crate::inference_bridge::Message {
                role: "user".to_string(),
                content: "Hello".to_string(),
            }],
            max_tokens: None,
            temperature: None,
            stream: Some(true),
        };

        let response = chat_completions(Extension(bridge), HeaderMap::new(), Json(request)).await;

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(value["error"]["type"], "invalid_request_error");
    }

    #[cfg(feature = "openai-compat")]
    #[tokio::test]
    async fn test_legacy_handler_prompt_matches_openai_compat_prompt_shape() {
        let config = OrchestratorConfig::default();
        let bridge = Arc::new(InferenceBridge::new(config));

        let legacy_request = ChatCompletionRequest {
            model: "gpt-4".to_string(),
            messages: vec![
                crate::inference_bridge::Message {
                    role: "system".to_string(),
                    content: "You are concise.".to_string(),
                },
                crate::inference_bridge::Message {
                    role: "user".to_string(),
                    content: "first".to_string(),
                },
                crate::inference_bridge::Message {
                    role: "assistant".to_string(),
                    content: "ack".to_string(),
                },
                crate::inference_bridge::Message {
                    role: "user".to_string(),
                    content: "second".to_string(),
                },
            ],
            max_tokens: None,
            temperature: None,
            stream: Some(false),
        };

        let compat_request = crate::openai_compat::types::ChatCompletionRequest {
            model: "gpt-4".to_string(),
            messages: vec![
                crate::openai_compat::types::ChatMessage {
                    role: "system".to_string(),
                    content: "You are concise.".to_string(),
                },
                crate::openai_compat::types::ChatMessage {
                    role: "user".to_string(),
                    content: "first".to_string(),
                },
                crate::openai_compat::types::ChatMessage {
                    role: "assistant".to_string(),
                    content: "ack".to_string(),
                },
                crate::openai_compat::types::ChatMessage {
                    role: "user".to_string(),
                    content: "second".to_string(),
                },
            ],
            stream: false,
            max_tokens: None,
            temperature: None,
            priority: crate::openai_compat::types::RequestPriorityParam::Normal,
        };

        let expected_prompt = compat_request.to_prompt();

        let response =
            chat_completions(Extension(bridge), HeaderMap::new(), Json(legacy_request)).await;
        let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let body: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert!(
            body["choices"][0]["message"]["content"]
                .as_str()
                .unwrap_or_default()
                .contains(&expected_prompt),
            "legacy handler prompt should match openai_compat formatting"
        );
    }
}
