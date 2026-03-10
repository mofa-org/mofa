//! OpenAI-compatible API handlers
//!
//! This module provides handlers for the OpenAI-compatible endpoints:
//! - POST /v1/chat/completions

use axum::{
    Json,
    extract::State,
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
};
use mofa_kernel::agent::context::AgentContext;
use mofa_kernel::agent::types::{AgentInput, AgentState};
use serde_json::json;
use std::sync::Arc;
use uuid::Uuid;

use crate::error::GatewayError;
use crate::state::AppState;
use crate::types::{ChatCompletionRequest, ChatCompletionResponse, Message, Role, Usage};

/// Extract client key from request headers for rate-limiting purposes
fn client_key(headers: &HeaderMap) -> String {
    headers
        .get("x-forwarded-for")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.split(',').next().unwrap_or(s).trim().to_string())
        .unwrap_or_else(|| "unknown".to_string())
}

/// POST /v1/chat/completions
///
/// OpenAI-compatible chat completions endpoint.
/// Routes the conversation to an agent and returns the response in OpenAI format.
pub async fn chat_completions(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(req): Json<ChatCompletionRequest>,
) -> Result<impl IntoResponse, GatewayError> {
    // Rate-limit check
    let client = client_key(&headers);
    if !state.rate_limiter.check(&client) {
        return Err(GatewayError::RateLimitExceeded(client));
    }

    // Reject streaming requests for now (not implemented)
    if req.stream {
        return Err(GatewayError::InvalidRequest(
            "Streaming is not implemented yet".to_string(),
        ));
    }

    // Extract the last user message
    let user_message = req
        .messages
        .iter()
        .rev()
        .find(|m| m.role == Role::User)
        .ok_or_else(|| GatewayError::InvalidRequest("No user message found".to_string()))?;

    // Determine which agent to use
    let agent_id = req.agent_id.clone();

    // If no agent_id specified, try to use the first available agent
    let target_agent_id = if let Some(id) = agent_id {
        id
    } else {
        // List agents and use the first one available
        let agents = state.registry.list().await;
        if agents.is_empty() {
            return Err(GatewayError::InvalidRequest(
                "No agents available. Please specify an agent_id or create an agent first."
                    .to_string(),
            ));
        }
        agents.first().unwrap().id.clone()
    };

    // Resolve agent
    let agent_arc = state
        .registry
        .get(&target_agent_id)
        .await
        .ok_or_else(|| GatewayError::AgentNotFound(target_agent_id.clone()))?;

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

    // Build input from the user message
    let input = AgentInput::text(user_message.content.clone());

    // Build execution context
    let execution_id = Uuid::new_v4().to_string();
    let session_id = Uuid::new_v4().to_string();
    let mut ctx = AgentContext::new(execution_id);
    ctx.session_id = Some(session_id.clone());

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
        agent_id = %target_agent_id,
        session_id = %session_id,
        duration_ms = duration_ms,
        "OpenAI chat completions request completed"
    );

    // Convert output to text
    let response_text = output.content.to_text();

    // Estimate token counts (rough approximation)
    let prompt_tokens = (user_message.content.len() / 4) as u32;
    let completion_tokens = (response_text.len() / 4) as u32;
    let usage = Usage::new(prompt_tokens, completion_tokens);

    // Build OpenAI-style response
    let response_id = format!("chatcmpl-{}", Uuid::new_v4().to_string()[..8].to_string());
    let assistant_message = Message {
        role: Role::Assistant,
        content: response_text,
        name: None,
    };

    let response = ChatCompletionResponse::new(response_id, req.model, assistant_message, usage);

    Ok((StatusCode::OK, Json(response)))
}

/// Build the OpenAI-compatible router sub-tree
pub fn openai_router() -> axum::Router<Arc<AppState>> {
    use axum::routing::post;
    axum::Router::new().route("/v1/chat/completions", post(chat_completions))
}
