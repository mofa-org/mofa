//! OpenAI-compatible mock LLM server for tests.
//!
//! Provides preset responses, response sequences, and error injection
//! for `/v1/chat/completions` requests.

use axum::{
    extract::State,
    http::StatusCode,
    routing::post,
    Json, Router,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::TcpListener;
use tokio::sync::{oneshot, RwLock};

#[derive(Debug, Clone)]
pub struct MockLlmServer {
    base_url: String,
    addr: SocketAddr,
    state: Arc<RwLock<ServerState>>,
    shutdown_tx: Option<oneshot::Sender<()>>,
}

#[derive(Debug, Default)]
struct ServerState {
    rules: Vec<Rule>,
    sequences: Vec<(String, VecDeque<RuleResponse>)>,
    default_response: String,
    history: Vec<RequestRecord>,
}

#[derive(Debug, Clone)]
struct Rule {
    prompt_substring: String,
    response: RuleResponse,
}

#[derive(Debug, Clone)]
enum RuleResponse {
    Text(String),
    Error { status: u16, message: String },
}

#[derive(Debug, Clone, Serialize)]
pub struct RequestRecord {
    pub received_at: DateTime<Utc>,
    pub model: Option<String>,
    pub prompt: String,
    pub raw: serde_json::Value,
}

#[derive(Debug, Deserialize, Serialize)]
struct ChatCompletionRequest {
    model: Option<String>,
    messages: Vec<ChatMessage>,
    #[serde(default)]
    stream: bool,
}

#[derive(Debug, Deserialize, Serialize)]
struct ChatMessage {
    role: Option<String>,
    content: Option<String>,
}

#[derive(Debug, Serialize)]
struct ChatCompletionResponse {
    id: String,
    object: String,
    created: u64,
    model: String,
    choices: Vec<ChatChoice>,
}

#[derive(Debug, Serialize)]
struct ChatChoice {
    index: usize,
    message: ChatMessageOut,
    finish_reason: String,
}

#[derive(Debug, Serialize)]
struct ChatMessageOut {
    role: String,
    content: String,
}

#[derive(Debug, Serialize)]
struct ErrorResponse {
    error: ErrorBody,
}

#[derive(Debug, Serialize)]
struct ErrorBody {
    message: String,
    r#type: String,
}

impl MockLlmServer {
    /// Start a mock LLM server on a random local port.
    pub async fn start() -> anyhow::Result<Self> {
        let state = Arc::new(RwLock::new(ServerState {
            rules: Vec::new(),
            sequences: Vec::new(),
            default_response: "Mock fallback response.".to_string(),
            history: Vec::new(),
        }));

        let app = Router::new()
            .route("/v1/chat/completions", post(handle_chat))
            .with_state(state.clone());

        let listener = TcpListener::bind("127.0.0.1:0").await?;
        let addr = listener.local_addr()?;
        let base_url = format!("http://{}", addr);

        let (shutdown_tx, shutdown_rx) = oneshot::channel();
        let server = axum::serve(listener, app).with_graceful_shutdown(async move {
            let _ = shutdown_rx.await;
        });
        tokio::spawn(server);

        Ok(Self {
            base_url,
            addr,
            state,
            shutdown_tx: Some(shutdown_tx),
        })
    }

    /// Base URL for HTTP clients (e.g., `http://127.0.0.1:PORT`).
    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    /// Socket address the server is bound to.
    pub fn addr(&self) -> SocketAddr {
        self.addr
    }

    /// Add a response rule matched by prompt substring.
    pub async fn add_response_rule(&self, prompt_substring: &str, response: &str) {
        let mut state = self.state.write().await;
        state.rules.push(Rule {
            prompt_substring: prompt_substring.to_string(),
            response: RuleResponse::Text(response.to_string()),
        });
    }

    /// Add an error rule matched by prompt substring.
    pub async fn add_error_rule(&self, prompt_substring: &str, status: u16, message: &str) {
        let mut state = self.state.write().await;
        state.rules.push(Rule {
            prompt_substring: prompt_substring.to_string(),
            response: RuleResponse::Error {
                status,
                message: message.to_string(),
            },
        });
    }

    /// Add a sequence of responses for a prompt substring.
    /// Each matching call consumes the next response; the last repeats.
    pub async fn add_response_sequence(&self, prompt_substring: &str, responses: Vec<&str>) {
        let mut state = self.state.write().await;
        let deque = responses.into_iter().map(|s| RuleResponse::Text(s.to_string())).collect();
        state.sequences.push((prompt_substring.to_string(), deque));
    }

    /// Set the default response when no rule matches.
    pub async fn set_default_response(&self, response: &str) {
        let mut state = self.state.write().await;
        state.default_response = response.to_string();
    }

    /// Retrieve a snapshot of request history.
    pub async fn history(&self) -> Vec<RequestRecord> {
        let state = self.state.read().await;
        state.history.clone()
    }

    /// Clear request history.
    pub async fn clear_history(&self) {
        let mut state = self.state.write().await;
        state.history.clear();
    }

    /// Shutdown the server.
    pub async fn shutdown(mut self) {
        if let Some(tx) = self.shutdown_tx.take() {
            let _ = tx.send(());
        }
    }
}

async fn handle_chat(
    State(state): State<Arc<RwLock<ServerState>>>,
    Json(payload): Json<ChatCompletionRequest>,
) -> Result<Json<ChatCompletionResponse>, (StatusCode, Json<ErrorResponse>)> {
    if payload.stream {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: ErrorBody {
                    message: "streaming not supported by MockLlmServer".to_string(),
                    r#type: "invalid_request_error".to_string(),
                },
            }),
        ));
    }

    let prompt = build_prompt(&payload.messages);
    let raw = serde_json::to_value(&payload).unwrap_or(serde_json::Value::Null);

    let response = {
        let mut guard = state.write().await;
        guard.history.push(RequestRecord {
            received_at: Utc::now(),
            model: payload.model.clone(),
            prompt: prompt.clone(),
            raw,
        });
        resolve_response(&mut guard, &prompt)
    };

    match response {
        RuleResponse::Text(text) => Ok(Json(ChatCompletionResponse {
            id: format!("mock-{}", uuid::Uuid::now_v7()),
            object: "chat.completion".to_string(),
            created: Utc::now().timestamp() as u64,
            model: payload
                .model
                .clone()
                .unwrap_or_else(|| "mock-model".to_string()),
            choices: vec![ChatChoice {
                index: 0,
                message: ChatMessageOut {
                    role: "assistant".to_string(),
                    content: text,
                },
                finish_reason: "stop".to_string(),
            }],
        })),
        RuleResponse::Error { status, message } => Err((
            StatusCode::from_u16(status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR),
            Json(ErrorResponse {
                error: ErrorBody {
                    message,
                    r#type: "mock_error".to_string(),
                },
            }),
        )),
    }
}

fn resolve_response(state: &mut ServerState, prompt: &str) -> RuleResponse {
    for (key, deque) in state.sequences.iter_mut() {
        if prompt.contains(key) {
            if deque.len() > 1 {
                return deque.pop_front().unwrap_or_else(|| {
                    RuleResponse::Text(state.default_response.clone())
                });
            }
            if let Some(last) = deque.front() {
                return last.clone();
            }
        }
    }

    for rule in state.rules.iter() {
        if prompt.contains(&rule.prompt_substring) {
            return rule.response.clone();
        }
    }

    RuleResponse::Text(state.default_response.clone())
}

fn build_prompt(messages: &[ChatMessage]) -> String {
    let mut parts = Vec::new();
    for msg in messages {
        let role = msg.role.as_deref().unwrap_or("unknown");
        let content = msg.content.as_deref().unwrap_or("");
        if !content.is_empty() {
            parts.push(format!("{}: {}", role, content));
        }
    }
    parts.join("\n")
}
