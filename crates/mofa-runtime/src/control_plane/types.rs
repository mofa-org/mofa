use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Request body for creating and registering a new agent.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CreateAgentRequest {
    /// Unique agent identifier.
    pub id: String,
    /// Human-readable agent name.
    pub name: String,
    /// Optional description shown in dashboards or logs.
    pub description: Option<String>,
    /// Factory or implementation type to instantiate.
    pub agent_type: String,
    /// Arbitrary configuration forwarded to the agent factory.
    #[serde(default)]
    pub config: Value,
}

/// Request body for invoking an agent through the gateway.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InvokeRequest {
    /// Target agent identifier.
    pub agent_id: String,
    /// Structured payload forwarded to the agent as input.
    #[serde(default)]
    pub payload: Value,
    /// Optional metadata for tracing or downstream routing.
    #[serde(default)]
    pub metadata: Value,
    /// Optional timeout in milliseconds for the invocation.
    #[serde(default)]
    pub timeout_ms: Option<u64>,
}

/// Standard API envelope for responses emitted by the control plane.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ApiResponse<T> {
    /// Indicates whether the request succeeded.
    pub success: bool,
    /// Response payload when successful.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<T>,
    /// Error details when unsuccessful.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<ApiError>,
}

impl<T> ApiResponse<T> {
    /// Construct a successful response containing `data`.
    pub fn ok(data: T) -> Self {
        Self {
            success: true,
            data: Some(data),
            error: None,
        }
    }

    /// Construct an error response.
    pub fn error(error: ApiError) -> Self {
        Self {
            success: false,
            data: None,
            error: Some(error),
        }
    }
}

/// API-level error type used by control plane handlers.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, thiserror::Error)]
#[non_exhaustive]
pub enum ApiError {
    /// Resource not found.
    #[error("not found: {0}")]
    NotFound(String),
    /// Invalid or malformed request payload.
    #[error("bad request: {0}")]
    BadRequest(String),
    /// Internal server error.
    #[error("internal error: {0}")]
    Internal(String),
}

impl ApiError {
    /// Human-readable message; shorthand over `ToString`.
    pub fn message(&self) -> String {
        self.to_string()
    }
}
