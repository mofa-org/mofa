use crate::llm::inference::{
    BackendHealth, HardwareRequirements, InferenceBackend, InferenceChunk, InferenceError,
    InferenceRequest, InferenceResponse, InferenceResult, InferenceStream, ModelCapability,
    ModelHandle, ModelMetadata, ModelType, QuantizationType, TokenUsage,
};
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

#[derive(Clone)]
pub struct LocalBackendConfig {
    pub models_dir: String,
    pub max_memory_gb: u32,
    pub enable_quantization: bool,
    pub default_quantization: QuantizationType,
}

impl Default for LocalBackendConfig {
    fn default() -> Self {
        Self {
            models_dir: \"./models\".to_string(),
            max_memory_gb: 16,
            enable_quantization: true,
            default_quantization: QuantizationType::Q8_0,
        }
    }
}

pub struct MlxLocalBackend {
    name: String,
    config: LocalBackendConfig,
    loaded_models: Mutex<HashMap<String, ModelHandle>>,
    model_registry: Mutex<HashMap<String, ModelMetadata>>,
}

impl MlxLocalBackend {
    pub fn new(config: LocalBackendConfig) -> Self {
        let mut backend = Self {
            name: \"mlx-local\".to_string(),
            config,
            loaded_models: Mutex::new(HashMap::new()),
            model_registry: Mutex::new(HashMap::new()),
        };
        backend.register_default_models();
        backend
    }

    fn register_default_models(&mut self) {
        let mut registry = self.model_registry.lock().unwrap();

        registry.insert(
            \"qwen2.5-0.5b\".to_string(),
            ModelMetadata {
                model_id: \"qwen2.5-0.5b\".to_string(),
                name: \"Qwen2.5 0.5B\".to_string(),
                provider: \"mlx\".to_string(),
                model_type: ModelType::LLM,
                context_length: 8192,
                capabilities: vec![ModelCapability::Streaming, ModelCapability::JsonMode],
                quantization: vec![QuantizationType::Q4_K, QuantizationType::Q8_0],
                hardware_requirements: HardwareRequirements {
                    min_memory_gb: 1,
                    recommended_memory_gb: 2,
                    gpu_required: false,
                    apple_silicon_optimized: true,
                },
            },
        );

        registry.insert(
            \"llama3-8b\".to_string(),
            ModelMetadata {
                model_id: \"llama3-8b\".to_string(),
                name: \"Llama 3 8B\".to_string(),
                provider: \"mlx\".to_string(),
                model_type: ModelType::LLM,
                context_length: 8192,
                capabilities: vec![ModelCapability::Streaming, ModelCapability::Tools],
                quantization: vec![QuantizationType::Q4_K, QuantizationType::Q5_K, QuantizationType::Q8_0],
                hardware_requirements: HardwareRequirements {
                    min_memory_gb: 4,
                    recommended_memory_gb: 8,
                    gpu_required: false,
                    apple_silicon_optimized: true,
                },
            },
        );

        registry.insert(
            \"funasr-base\".to_string(),
            ModelMetadata {
                model_id: \"funasr-base\".to_string(),
                name: \"FunASR Base\".to_string(),
                provider: \"mlx\".to_string(),
                model_type: ModelType::ASR,
                context_length: 480000,
                capabilities: vec![ModelCapability::Streaming],
                quantization: vec![QuantizationType::Q8_0, QuantizationType::F16],
                hardware_requirements: HardwareRequirements {
                    min_memory_gb: 1,
                    recommended_memory_gb: 2,
                    gpu_required: false,
                    apple_silicon_optimized: true,
                },
            },
        );

        registry.insert(
            \"gpt-sovits\".to_string(),
            ModelMetadata {
                model_id: \"gpt-sovits\".to_string(),
                name: \"GPT-SoVITS\".to_string(),
                provider: \"mlx\".to_string(),
                model_type: ModelType::TTS,
                context_length: 1024,
                capabilities: vec![ModelCapability::Streaming],
                quantization: vec![QuantizationType::Q8_0, QuantizationType::F16],
                hardware_requirements: HardwareRequirements {
                    min_memory_gb: 2,
                    recommended_memory_gb: 4,
                    gpu_required: false,
                    apple_silicon_optimized: true,
                },
            },
        );
    }
}

#[async_trait]
impl InferenceBackend for MlxLocalBackend {
    fn name(&self) -> &str {
        &self.name
    }

    async fn load_model(&self, model_id: &str) -> InferenceResult<ModelHandle> {
        let registry = self.model_registry.lock().unwrap();

        if !registry.contains_key(model_id) {
            return Err(InferenceError::ModelNotFound(model_id.to_string()));
        }

        drop(registry);

        let handle = ModelHandle {
            model_id: model_id.to_string(),
            backend: Arc::new(self.clone()),
        };

        let mut loaded = self.loaded_models.lock().unwrap();
        loaded.insert(model_id.to_string(), handle.clone());

        Ok(handle)
    }

    async fn unload_model(&self, model_id: &str) -> InferenceResult<()> {
        let mut loaded = self.loaded_models.lock().unwrap();
        loaded.remove(model_id);
        Ok(())
    }

    async fn generate(&self, request: InferenceRequest) -> InferenceResult<InferenceResponse> {
        let loaded = self.loaded_models.lock().unwrap();

        if !loaded.contains_key(&request.model_id) {
            return Err(InferenceError::ModelNotLoaded(request.model_id));
        }

        drop(loaded);

        let mock_content = format!(
            \"Generated response for: {}\",
            &request.prompt[..request.prompt.len().min(50)]
        );

        Ok(InferenceResponse {
            model_id: request.model_id,
            content: mock_content,
            usage: TokenUsage {
                prompt_tokens: 100,
                completion_tokens: 50,
                total_tokens: 150,
            },
            finish_reason: Some(\"stop\".to_string()),
        })
    }

    async fn generate_stream(&self, request: InferenceRequest) -> InferenceResult<InferenceStream> {
        let loaded = self.loaded_models.lock().unwrap();

        if !loaded.contains_key(&request.model_id) {
            return Err(InferenceError::ModelNotLoaded(request.model_id));
        }

        drop(loaded);

        let model_id = request.model_id.clone();
        let content = format!(
            \"Streaming response for: {}\",
            &request.prompt[..request.prompt.len().min(50)]
        );

        Ok(Box::pin(async_stream::try_stream! {
            for word in content.split_whitespace() {
                yield Ok(InferenceChunk {
                    model_id: model_id.clone(),
                    content: word.to_string(),
                    delta: Some(word.to_string()),
                    usage: None,
                    finish_reason: None,
                });
                tokio::time::sleep(std::time::Duration::from_millis(50)).await;
            }

            yield Ok(InferenceChunk {
                model_id,
                content: String::new(),
                delta: None,
                usage: Some(TokenUsage {
                    prompt_tokens: 100,
                    completion_tokens: 50,
                    total_tokens: 150,
                }),
                finish_reason: Some(\"stop\".to_string()),
            });
        }))
    }

    async fn list_models(&self) -> InferenceResult<Vec<ModelMetadata>> {
        let registry = self.model_registry.lock().unwrap();
        Ok(registry.values().cloned().collect())
    }

    async fn get_model(&self, model_id: &str) -> InferenceResult<ModelMetadata> {
        let registry = self.model_registry.lock().unwrap();
        registry
            .get(model_id)
            .cloned()
            .ok_or_else(|| InferenceError::ModelNotFound(model_id.to_string()))
    }

    async fn health_check(&self) -> InferenceResult<BackendHealth> {
        let loaded = self.loaded_models.lock().unwrap();
        let loaded_models: Vec<String> = loaded.keys().cloned().collect();

        Ok(BackendHealth {
            healthy: true,
            latency_ms: 10,
            loaded_models,
            available_memory_gb: 8,
        })
    }

    fn capabilities(&self) -> HashMap<String, bool> {
        let mut caps = HashMap::new();
        caps.insert(\"streaming\".to_string(), true);
        caps.insert(\"quantization\".to_string(), true);
        caps.insert(\"apple_silicon\".to_string(), true);
        caps.insert(\"zero_copy\".to_string(), true);
        caps
    }
}
