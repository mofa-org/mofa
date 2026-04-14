//! Persisted-memory artifact example.
//!
//! Runs two sessions against a real SQLite store, reloads one session from a
//! fresh store handle, and prints a JSON artifact similar to testing outputs.

use std::sync::Arc;

use anyhow::Result;
use async_trait::async_trait;
use mofa_foundation::llm::{
    ChatCompletionRequest, ChatCompletionResponse, ChatMessage, ChatSession, Choice,
    EmbeddingData, EmbeddingInput, EmbeddingRequest, EmbeddingResponse, EmbeddingUsage,
    LLMClient, LLMProvider, LLMResult, MessageContent, Role,
};
use mofa_foundation::persistence::SqliteStore;
use serde::Serialize;

#[derive(Debug, Serialize)]
struct PersistedMemoryArtifact {
    session_id: String,
    history: Vec<String>,
}

#[tokio::main]
async fn main() -> Result<()> {
    // Use a temp SQLite DB so the example stays local and isolated.
    let tmp = tempfile::tempdir()?;
    let db_path = tmp.path().join("persisted-memory-example.db");
    std::fs::File::create(&db_path)?;
    let database_url = format!("sqlite:{}", db_path.to_string_lossy());

    let provider = Arc::new(ExampleMockProvider::new("fixture-model"));
    let user_id = uuid::Uuid::now_v7();
    let tenant_id = uuid::Uuid::now_v7();
    let agent_id = uuid::Uuid::now_v7();

    let alpha_session_id = uuid::Uuid::now_v7();
    let beta_session_id = uuid::Uuid::now_v7();

    let writer_store = Arc::new(SqliteStore::connect(&database_url).await?);

    // Seed the primary session we will reload and print.
    run_and_persist_single_turn(
        provider.clone(),
        writer_store.clone(),
        user_id,
        tenant_id,
        agent_id,
        alpha_session_id,
        "session-alpha: I forgot my password and cannot log in.",
    )
    .await?;

    // Seed a second session to show cross-session data can coexist safely.
    run_and_persist_single_turn(
        provider.clone(),
        writer_store,
        user_id,
        tenant_id,
        agent_id,
        beta_session_id,
        "session-beta: What is the refund policy for annual plans?",
    )
    .await?;

    // Fresh connection simulates process/session restart before reload.
    let reloaded_store = Arc::new(SqliteStore::connect(&database_url).await?);
    let reloaded_alpha = ChatSession::load(
        alpha_session_id,
        LLMClient::new(provider),
        user_id,
        tenant_id,
        agent_id,
        reloaded_store.clone(),
        reloaded_store,
        None,
    )
    .await?;

    let artifact = PersistedMemoryArtifact {
        session_id: alpha_session_id.to_string(),
        history: reloaded_alpha
            .messages()
            .iter()
            .filter_map(|message| match &message.content {
                Some(MessageContent::Text(text)) => Some(text.clone()),
                _ => None,
            })
            .collect(),
    };

    // Print the artifact in the same shape used by persisted-memory tests.
    println!("{}", serde_json::to_string_pretty(&artifact)?);
    Ok(())
}

async fn run_and_persist_single_turn(
    provider: Arc<ExampleMockProvider>,
    store: Arc<SqliteStore>,
    user_id: uuid::Uuid,
    tenant_id: uuid::Uuid,
    agent_id: uuid::Uuid,
    session_id: uuid::Uuid,
    prompt: &str,
) -> Result<()> {
    // Drive the real send() + save() path instead of manual message injection.
    let mut session = ChatSession::with_id_and_stores(
        session_id,
        LLMClient::new(provider),
        user_id,
        tenant_id,
        agent_id,
        store.clone(),
        store,
        None,
    );

    let _ = session.send(prompt).await?;
    session.save().await?;
    Ok(())
}

struct ExampleMockProvider {
    default_model_name: String,
}

impl ExampleMockProvider {
    fn new(default_model_name: &str) -> Self {
        Self {
            default_model_name: default_model_name.to_string(),
        }
    }
}

#[async_trait]
impl LLMProvider for ExampleMockProvider {
    fn name(&self) -> &str {
        "persisted-memory-example-mock"
    }

    fn default_model(&self) -> &str {
        &self.default_model_name
    }

    async fn chat(&self, request: ChatCompletionRequest) -> LLMResult<ChatCompletionResponse> {
        // Keep output deterministic by deriving it from the latest user prompt.
        let response_content = request
            .messages
            .iter()
            .rev()
            .find_map(|msg| match (&msg.role, &msg.content) {
                (Role::User, Some(MessageContent::Text(text))) => {
                    Some(format!("I can help with this issue: {text}"))
                }
                _ => None,
            })
            .unwrap_or_else(|| "I can help with this issue.".to_string());

        Ok(ChatCompletionResponse {
            id: "resp-1".to_string(),
            object: "chat.completion".to_string(),
            created: 1,
            model: self.default_model_name.clone(),
            choices: vec![Choice {
                index: 0,
                message: ChatMessage {
                    role: Role::Assistant,
                    content: Some(MessageContent::Text(response_content)),
                    name: None,
                    tool_calls: None,
                    tool_call_id: None,
                },
                finish_reason: None,
                logprobs: None,
            }],
            usage: None,
            system_fingerprint: None,
        })
    }

    async fn embedding(&self, request: EmbeddingRequest) -> LLMResult<EmbeddingResponse> {
        let data = match request.input {
            EmbeddingInput::Single(_) => vec![EmbeddingData {
                object: "embedding".to_string(),
                index: 0,
                embedding: vec![0.1, 0.2],
            }],
            EmbeddingInput::Multiple(values) => values
                .iter()
                .enumerate()
                .map(|(idx, _)| EmbeddingData {
                    object: "embedding".to_string(),
                    index: idx as u32,
                    embedding: vec![idx as f32],
                })
                .collect(),
        };

        Ok(EmbeddingResponse {
            object: "list".to_string(),
            model: self.default_model_name.clone(),
            data,
            usage: EmbeddingUsage {
                prompt_tokens: 1,
                total_tokens: 1,
            },
        })
    }
}
