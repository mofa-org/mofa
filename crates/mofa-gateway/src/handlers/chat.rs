//! Chat routing endpoint
//!
//! POST /agents/{id}/chat - send a message to a specific agent and return its response

use axum::{
    Json,
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
};
use mofa_kernel::agent::context::AgentContext;
use mofa_kernel::agent::types::{AgentInput, AgentState};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::sync::Arc;
use uuid::Uuid;

use crate::error::GatewayError;
use crate::state::AppState;

/// Request body for POST /agents/{id}/chat
#[derive(Debug, Deserialize)]
pub struct ChatRequest {
    /// Plain-text message sent to the agent
    pub message: Option<String>,
    /// Structured JSON payload (mutually exclusive with `message`; takes
    /// precedence when both are set)
    pub data: Option<Value>,
    /// Optional session ID for multi-turn conversations
    pub session_id: Option<String>,
}

/// Response returned by the chat endpoint
#[derive(Debug, Serialize)]
pub struct ChatResponse {
    /// ID of the agent that processed the request.
    pub agent_id: String,
    /// Session identifier for multi-turn conversations.
    pub session_id: String,
    /// Agent output as a JSON value.
    pub output: Value,
    /// Request processing time in milliseconds.
    pub duration_ms: u64,
    /// Whether this response was returned from semantic cache.
    pub cache_hit: bool,
    /// Similarity score for semantic cache hits.
    pub cache_similarity: Option<f32>,
}

/// Extract client key from request headers for rate-limiting purposes
fn client_key(headers: &HeaderMap) -> String {
    headers
        .get("x-forwarded-for")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.split(',').next().unwrap_or(s).trim().to_string())
        .unwrap_or_else(|| "unknown".to_string())
}

/// POST /agents/{id}/chat
///
/// Routes an incoming message to the specified agent and returns the result.
/// The agent must be in `Ready` or `Running` state; otherwise 409 is returned.
pub async fn chat(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    headers: HeaderMap,
    Json(req): Json<ChatRequest>,
) -> Result<impl IntoResponse, GatewayError> {
    // Rate-limit check
    let client = client_key(&headers);
    if !state.rate_limiter.check(&client) {
        return Err(GatewayError::RateLimitExceeded(client));
    }

    // Resolve agent
    let agent_arc = state
        .registry
        .get(&id)
        .await
        .ok_or_else(|| GatewayError::AgentNotFound(id.clone()))?;

    // Validate state before executing
    {
        let agent = agent_arc.read().await;
        let current = agent.state();
        if current != AgentState::Ready && current != AgentState::Running {
            return Err(GatewayError::AgentOperationFailed(format!(
                "agent is in state '{}' and cannot process messages",
                current
            )));
        }
    }

    let raw_message = req.message.clone();

    // Build input
    let input = match req.data {
        Some(json_data) => AgentInput::json(json_data),
        None => match req.message {
            Some(msg) => AgentInput::text(msg),
            None => AgentInput::Empty,
        },
    };

    // Build execution context
    let execution_id = Uuid::new_v4().to_string();
    let session_id = req.session_id.unwrap_or_else(|| Uuid::new_v4().to_string());
    let mut ctx = AgentContext::new(execution_id);
    ctx.session_id = Some(session_id.clone());

    if let Some(prompt) = raw_message.as_deref()
        && let Some(hit) = state
            .semantic_cache
            .lookup(&id, prompt)
            .await
            .map_err(|e| GatewayError::Internal(format!("semantic cache lookup failed: {e}")))?
    {
        tracing::debug!(
            agent_id = %id,
            session_id = %session_id,
            score = hit.score,
            "semantic cache hit"
        );

        let response = ChatResponse {
            agent_id: id,
            session_id,
            output: hit.output,
            duration_ms: 0,
            cache_hit: true,
            cache_similarity: Some(hit.score),
        };

        return Ok((StatusCode::OK, Json(response)));
    }

    // Execute
    let start = std::time::Instant::now();
    let output = {
        let mut agent = agent_arc.write().await;
        agent
            .execute(input, &ctx)
            .await
            .map_err(|e| GatewayError::AgentOperationFailed(e.to_string()))?
    };
    let duration_ms = start.elapsed().as_millis() as u64;

    tracing::info!(
        agent_id = %id,
        session_id = %session_id,
        duration_ms = duration_ms,
        "chat request completed"
    );

    let output_value = serde_json::to_value(&output.content)
        .unwrap_or_else(|_| json!(output.content.to_text()));

    let response = ChatResponse {
        agent_id: id.clone(),
        session_id,
        output: output_value.clone(),
        duration_ms,
        cache_hit: false,
        cache_similarity: None,
    };

    if let Some(prompt) = raw_message.as_deref() {
        state
            .semantic_cache
            .insert(&id, prompt, &output_value)
            .await
            .map_err(|e| GatewayError::Internal(format!("semantic cache write failed: {e}")))?;
    }

    Ok((StatusCode::OK, Json(response)))
}

/// Build the chat router sub-tree
pub fn chat_router() -> axum::Router<Arc<AppState>> {
    use axum::routing::post;
    axum::Router::new().route("/agents/{id}/chat", post(chat))
}
