//! Shared persisted-memory test support for SQLite-backed integration paths.

use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use mofa_foundation::llm::{
    ChatCompletionRequest, ChatCompletionResponse, ChatMessage, ChatSession, Choice,
    EmbeddingData, EmbeddingInput, EmbeddingRequest, EmbeddingResponse, EmbeddingUsage,
    LLMClient, LLMProvider, LLMResult, MessageContent, Role,
};
use mofa_foundation::persistence::{PersistenceError, PersistenceResult, SqliteStore};
use serde::{Deserialize, Serialize};
use tempfile::tempdir;

pub struct PersistedMemoryFixture {
    _dir: tempfile::TempDir,
    database_url: String,
    provider: Arc<FixtureMockProvider>,
    user_id: uuid::Uuid,
    tenant_id: uuid::Uuid,
    agent_id: uuid::Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersistedMemoryArtifact {
    pub session_id: String,
    pub history: Vec<String>,
}

impl PersistedMemoryFixture {
    pub fn new(name: &str) -> Self {
        let dir = tempdir().expect("tempdir");
        let path = dir.path().join(name);
        std::fs::File::create(&path).expect("sqlite file should be created");

        Self {
            _dir: dir,
            database_url: format!("sqlite:{}", path.to_string_lossy()),
            provider: Arc::new(FixtureMockProvider::new("fixture-model", Some("ok"))),
            user_id: uuid::Uuid::now_v7(),
            tenant_id: uuid::Uuid::now_v7(),
            agent_id: uuid::Uuid::now_v7(),
        }
    }

    pub async fn open_store(&self) -> Arc<SqliteStore> {
        // Open a fresh connection so tests can model restart/reload boundaries explicitly.
        Arc::new(
            SqliteStore::connect(&self.database_url)
                .await
                .expect("sqlite store should connect"),
        )
    }

    pub fn new_session_id(&self) -> uuid::Uuid {
        uuid::Uuid::now_v7()
    }

    pub async fn write_session(
        &self,
        store: Arc<SqliteStore>,
        session_id: uuid::Uuid,
        messages: &[(&str, &str)],
    ) {
        // Seed persisted session state through the real ChatSession + SQLite path.
        let mut session = ChatSession::with_id_and_stores(
            session_id,
            LLMClient::new(self.provider.clone()),
            self.user_id,
            self.tenant_id,
            self.agent_id,
            store.clone(),
            store,
            None,
        );

        for (role, content) in messages {
            match *role {
                "user" => session.messages_mut().push(ChatMessage::user(*content)),
                "assistant" => session.messages_mut().push(ChatMessage::assistant(*content)),
                other => panic!("unsupported role in test fixture: {other}"),
            }
        }

        session.save().await.expect("session should persist");
    }

    pub async fn reload_session(
        &self,
        store: Arc<SqliteStore>,
        session_id: uuid::Uuid,
    ) -> ChatSession {
        // Reload persisted history as a fresh consumer would see it.
        ChatSession::load(
            session_id,
            LLMClient::new(self.provider.clone()),
            self.user_id,
            self.tenant_id,
            self.agent_id,
            store.clone(),
            store,
            None,
        )
        .await
        .expect("session should reload")
    }

    pub async fn reload_session_result(
        &self,
        store: Arc<SqliteStore>,
        session_id: uuid::Uuid,
    ) -> PersistenceResult<ChatSession> {
        ChatSession::load(
            session_id,
            LLMClient::new(self.provider.clone()),
            self.user_id,
            self.tenant_id,
            self.agent_id,
            store.clone(),
            store,
            None,
        )
        .await
    }
}

pub fn assert_persisted_session_exists(session: &ChatSession) {
    // A persisted session should reload with some observable history.
    assert!(
        !session.messages().is_empty(),
        "reloaded persisted session should contain message history"
    );
}

pub fn assert_reloaded_history_len(session: &ChatSession, expected: usize) {
    assert_eq!(session.messages().len(), expected);
}

pub fn assert_reloaded_history_contains(session: &ChatSession, index: usize, expected: &str) {
    assert_eq!(message_text(&session.messages()[index]), Some(expected));
}

pub fn assert_no_cross_session_leakage(session: &ChatSession, forbidden_marker: &str) {
    // This guards against session contamination within the same backing store.
    assert!(
        session
            .messages()
            .iter()
            .all(|msg| message_text(msg).map(|text| !text.contains(forbidden_marker)).unwrap_or(true)),
        "reloaded history should not contain content marked with {forbidden_marker}"
    );
}

pub fn assert_missing_persisted_session(result: PersistenceResult<ChatSession>) {
    // Missing persisted state should fail as NotFound, not as a silent empty history.
    match result {
        Err(PersistenceError::NotFound(_)) => {}
        Err(other) => panic!("expected not found error, got {other:?}"),
        Ok(_) => panic!("expected persisted session lookup to fail"),
    }
}

pub fn build_persisted_memory_artifact(
    session_id: uuid::Uuid,
    session: &ChatSession,
) -> PersistedMemoryArtifact {
    let history = session
        .messages()
        .iter()
        .filter_map(message_text)
        .map(|s| s.to_string())
        .collect::<Vec<_>>();

    PersistedMemoryArtifact {
        session_id: session_id.to_string(),
        history,
    }
}

pub fn render_artifact_json(artifact: &PersistedMemoryArtifact) -> String {
    serde_json::to_string_pretty(artifact).expect("artifact should serialize to json")
}

pub fn assert_artifact_json_output_has_core_fields(json: &str) {
    let parsed: serde_json::Value = serde_json::from_str(json).expect("artifact json should parse");
    assert!(
        parsed.get("session_id").and_then(|v| v.as_str()).is_some(),
        "artifact json should contain string session_id"
    );
    assert!(
        parsed.get("history").and_then(|v| v.as_array()).is_some(),
        "artifact json should contain history array"
    );
}

pub fn assert_artifact_history_len(artifact: &PersistedMemoryArtifact, expected: usize) {
    assert_eq!(artifact.history.len(), expected);
}

pub fn assert_artifact_history_contains(
    artifact: &PersistedMemoryArtifact,
    index: usize,
    expected: &str,
) {
    assert_eq!(artifact.history.get(index).map(String::as_str), Some(expected));
}

pub fn assert_artifact_no_cross_session_leakage(
    artifact: &PersistedMemoryArtifact,
    forbidden_marker: &str,
) {
    assert!(
        artifact
            .history
            .iter()
            .all(|text| !text.contains(forbidden_marker)),
        "artifact history should not contain content marked with {forbidden_marker}"
    );
}

fn message_text(message: &ChatMessage) -> Option<&str> {
    match &message.content {
        Some(MessageContent::Text(text)) => Some(text.as_str()),
        _ => None,
    }
}

pub struct FixtureMockProvider {
    default_model_name: String,
    last_request: Arc<Mutex<Option<ChatCompletionRequest>>>,
    response_content: Option<String>,
}

impl FixtureMockProvider {
    fn new(default_model_name: &str, response_content: Option<&str>) -> Self {
        Self {
            default_model_name: default_model_name.to_string(),
            last_request: Arc::new(Mutex::new(None)),
            response_content: response_content.map(|s| s.to_string()),
        }
    }
}

#[async_trait]
impl LLMProvider for FixtureMockProvider {
    fn name(&self) -> &str {
        "fixture-mock"
    }

    fn default_model(&self) -> &str {
        &self.default_model_name
    }

    async fn chat(&self, request: ChatCompletionRequest) -> LLMResult<ChatCompletionResponse> {
        *self.last_request.lock().expect("lock poisoned") = Some(request);

        let message = ChatMessage {
            role: Role::Assistant,
            content: self
                .response_content
                .as_ref()
                .map(|s| MessageContent::Text(s.clone())),
            name: None,
            tool_calls: None,
            tool_call_id: None,
        };

        Ok(ChatCompletionResponse {
            id: "resp-1".to_string(),
            object: "chat.completion".to_string(),
            created: 1,
            model: self.default_model_name.clone(),
            choices: vec![Choice {
                index: 0,
                message,
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
