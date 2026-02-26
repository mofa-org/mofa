use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InferenceRequest {
    pub model_id: String,
    pub prompt: String,
    pub max_tokens: Option<u32>,
    pub temperature: Option<f32>,
    pub stream: bool,
    pub tools: Option<Vec<ToolDefinition>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InferenceResponse {
    pub model_id: String,
    pub content: String,
    pub usage: TokenUsage,
    pub finish_reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenUsage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    pub parameters: serde_json::Value,
}

pub type InferenceStream = futures::stream::BoxStream<'static, Result<InferenceChunk, InferenceError>>;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InferenceChunk {
    pub model_id: String,
    pub content: String,
    pub delta: Option<String>,
    pub usage: Option<TokenUsage>,
    pub finish_reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelMetadata {
    pub model_id: String,
    pub name: String,
    pub provider: String,
    pub model_type: ModelType,
    pub context_length: u32,
    pub capabilities: Vec<ModelCapability>,
    pub quantization: Vec<QuantizationType>,
    pub hardware_requirements: HardwareRequirements,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ModelType {
    LLM,
    ASR,
    TTS,
    Embedding,
    Vision,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ModelCapability {
    Streaming,
    Tools,
    Vision,
    Embedding,
    JsonMode,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum QuantizationType {
    Q4_K,
    Q5_K,
    Q8_0,
    F16,
    F32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HardwareRequirements {
    pub min_memory_gb: u32,
    pub recommended_memory_gb: u32,
    pub gpu_required: bool,
    pub apple_silicon_optimized: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackendHealth {
    pub healthy: bool,
    pub latency_ms: u64,
    pub loaded_models: Vec<String>,
    pub available_memory_gb: u32,
}

#[derive(Debug, thiserror::Error, Clone)]
pub enum InferenceError {
    #[error(\"Model not found: {0}\")]
    ModelNotFound(String),

    #[error(\"Model not loaded: {0}\")]
    ModelNotLoaded(String),

    #[error(\"Backend error: {0}\")]
    BackendError(String),

    #[error(\"Invalid request: {0}\")]
    InvalidRequest(String),

    #[error(\"Rate limited: {0}\")]
    RateLimited(String),

    #[error(\"Timeout: {0}\")]
    Timeout(String),

    #[error(\"Resource exhausted: {0}\")]
    ResourceExhausted(String),
}

pub type InferenceResult<T> = Result<T, InferenceError>;

#[async_trait]
pub trait InferenceBackend: Send + Sync {
    fn name(&self) -> &str;

    async fn load_model(&self, model_id: &str) -> InferenceResult<ModelHandle>;

    async fn unload_model(&self, model_id: &str) -> InferenceResult<()>;

    async fn generate(&self, request: InferenceRequest) -> InferenceResult<InferenceResponse>;

    async fn generate_stream(&self, request: InferenceRequest) -> InferenceResult<InferenceStream>;

    async fn list_models(&self) -> InferenceResult<Vec<ModelMetadata>>;

    async fn get_model(&self, model_id: &str) -> InferenceResult<ModelMetadata>;

    async fn health_check(&self) -> InferenceResult<BackendHealth>;

    fn capabilities(&self) -> HashMap<String, bool>;
}

#[derive(Clone)]
pub struct ModelHandle {
    pub model_id: String,
    pub backend: Arc<dyn InferenceBackend>,
}

impl ModelHandle {
    pub async fn generate(&self, request: InferenceRequest) -> InferenceResult<InferenceResponse> {
        self.backend.generate(request).await
    }

    pub async fn generate_stream(&self, request: InferenceRequest) -> InferenceResult<InferenceStream> {
        self.backend.generate_stream(request).await
    }
}
