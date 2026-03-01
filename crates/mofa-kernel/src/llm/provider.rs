use async_trait::async_trait;
use futures::Stream;
use std::pin::Pin;

use crate::agent::AgentResult;
use super::types::*;

/// Streaming response type
pub type ChatStream =
    Pin<Box<dyn Stream<Item = AgentResult<ChatCompletionChunk>> + Send>>;

/// Canonical LLM Provider trait (Kernel-owned)
#[async_trait]
pub trait LLMProvider: Send + Sync {
    /// Provider name
    fn name(&self) -> &str;

    /// Default model
    fn default_model(&self) -> &str {
        ""
    }

    /// Supported models
    fn supported_models(&self) -> Vec<&str> {
        vec![]
    }

    /// Supports streaming?
    fn supports_streaming(&self) -> bool {
        true
    }

    /// Supports tool calling?
    fn supports_tools(&self) -> bool {
        true
    }

    /// Supports vision?
    fn supports_vision(&self) -> bool {
        false
    }

    /// Supports embedding?
    fn supports_embedding(&self) -> bool {
        false
    }

    /// Chat request
    async fn chat(
        &self,
        request: ChatCompletionRequest,
    ) -> AgentResult<ChatCompletionResponse>;

    /// Streaming chat (default: not supported)
    async fn chat_stream(
        &self,
        _request: ChatCompletionRequest,
    ) -> AgentResult<ChatStream> {
        Err(crate::agent::AgentError::Other(format!(
            "Provider {} does not support streaming",
            self.name()
        )))
    }

    /// Embedding request
    async fn embedding(
        &self,
        _request: EmbeddingRequest,
    ) -> AgentResult<EmbeddingResponse> {
        Err(crate::agent::AgentError::Other(format!(
            "Provider {} does not support embedding",
            self.name()
        )))
    }

    /// Health check
    async fn health_check(&self) -> AgentResult<bool> {
        Ok(true)
    }

    /// Model info
    async fn get_model_info(
        &self,
        _model: &str,
    ) -> AgentResult<ModelInfo> {
        Err(crate::agent::AgentError::Other(format!(
            "Provider {} does not support model info",
            self.name()
        )))
    }
}

/// Model information
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ModelInfo {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub context_window: Option<u32>,
    pub max_output_tokens: Option<u32>,
    pub training_cutoff: Option<String>,
    pub capabilities: ModelCapabilities,
}

/// Model capabilities
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct ModelCapabilities {
    pub streaming: bool,
    pub tools: bool,
    pub vision: bool,
    pub json_mode: bool,
    pub json_schema: bool,
}