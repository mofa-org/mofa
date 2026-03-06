//! Gateway error types

use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;
use thiserror::Error;

/// Gateway-level errors
#[derive(Debug, Error)]
pub enum GatewayError {
    #[error("agent not found: {0}")]
    AgentNotFound(String),

    #[error("agent already exists: {0}")]
    AgentAlreadyExists(String),

    #[error("rate limit exceeded for client {0}")]
    RateLimitExceeded(String),

    #[error("agent operation failed: {0}")]
    AgentOperationFailed(String),

    #[error("invalid request: {0}")]
    InvalidRequest(String),

    #[error("internal error: {0}")]
    Internal(String),
}

impl IntoResponse for GatewayError {
    fn into_response(self) -> Response {
        let (status, code, message) = match &self {
            GatewayError::AgentNotFound(id) => (
                StatusCode::NOT_FOUND,
                "AGENT_NOT_FOUND",
                format!("agent '{}' not found", id),
            ),
            GatewayError::AgentAlreadyExists(id) => (
                StatusCode::CONFLICT,
                "AGENT_ALREADY_EXISTS",
                format!("agent '{}' already exists", id),
            ),
            GatewayError::RateLimitExceeded(client) => (
                StatusCode::TOO_MANY_REQUESTS,
                "RATE_LIMIT_EXCEEDED",
                format!("rate limit exceeded for client '{}'", client),
            ),
            GatewayError::AgentOperationFailed(msg) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "AGENT_OPERATION_FAILED",
                msg.clone(),
            ),
            GatewayError::InvalidRequest(msg) => (
                StatusCode::BAD_REQUEST,
                "INVALID_REQUEST",
                msg.clone(),
            ),
            GatewayError::Internal(msg) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "INTERNAL_ERROR",
                msg.clone(),
            ),
        };

        let body = Json(json!({
            "error": {
                "code": code,
                "message": message,
            }
        }));

        (status, body).into_response()
    }
}

pub type GatewayResult<T> = Result<T, GatewayError>;
