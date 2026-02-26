use crate::llm::inference::{InferenceBackend, InferenceError, InferenceResult, InferenceRequest, InferenceStream};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::RwLock;
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PipelineStage {
    ASR,
    LLM,
    TTS,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PipelineConfig {
    pub stages: Vec<PipelineStageConfig>,
    pub timeout_ms: u64,
    pub enable_streaming: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PipelineStageConfig {
    pub stage_type: PipelineStage,
    pub model_id: String,
    pub backend: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PipelineMetrics {
    pub stage_latencies_ms: Vec<u64>,
    pub total_latency_ms: u64,
    pub stages_completed: usize,
    pub stages_failed: usize,
}

pub struct InferencePipeline {
    config: PipelineConfig,
    backends: std::collections::HashMap<String, Arc<dyn InferenceBackend>>,
    metrics: RwLock<PipelineMetrics>,
}

impl InferencePipeline {
    pub fn new(config: PipelineConfig) -> Self {
        Self {
            config,
            backends: std::collections::HashMap::new(),
            metrics: RwLock::new(PipelineMetrics {
                stage_latencies_ms: vec![],
                total_latency_ms: 0,
                stages_completed: 0,
                stages_failed: 0,
            }),
        }
    }

    pub fn register_backend(&mut self, name: String, backend: Arc<dyn InferenceBackend>) {
        self.backends.insert(name, backend);
    }

    pub async fn execute(&self, input: String) -> InferenceResult<String> {
        let start = Instant::now();
        let mut current_input = input;
        let mut stage_latencies = Vec::new();
        let mut stages_completed = 0;
        let mut stages_failed = 0;

        for stage_config in &self.config.stages {
            let stage_start = Instant::now();

            let backend = self.backends
                .get(&stage_config.backend)
                .ok_or_else(|| InferenceError::BackendError(format!(
                    \"Backend not found: {}\", stage_config.backend
                )))?;

            let request = InferenceRequest {
                model_id: stage_config.model_id.clone(),
                prompt: current_input,
                max_tokens: Some(2048),
                temperature: Some(0.7),
                stream: false,
                tools: None,
            };

            match backend.generate(request).await {
                Ok(response) => {
                    let latency = stage_start.elapsed().as_millis() as u64;
                    stage_latencies.push(latency);
                    current_input = response.content;
                    stages_completed += 1;
                }
                Err(e) => {
                    stages_failed += 1;
                    return Err(e);
                }
            }
        }

        let total_latency = start.elapsed().as_millis() as u64;

        let mut metrics = self.metrics.write().await;
        metrics.stage_latencies_ms = stage_latencies;
        metrics.total_latency_ms = total_latency;
        metrics.stages_completed = stages_completed;
        metrics.stages_failed = stages_failed;

        Ok(current_input)
    }

    pub async fn execute_stream(&self, input: String) -> InferenceResult<InferenceStream> {
        let config = self.config.clone();
        let backends = self.backends.clone();

        Ok(Box::pin(async_stream::try_stream! {
            let mut current_input = input;

            for stage_config in &config.stages {
                let backend = backends
                    .get(&stage_config.backend)
                    .ok_or_else(|| InferenceError::BackendError(format!(
                        \"Backend not found: {}\", stage_config.backend
                    )))?;

                let request = InferenceRequest {
                    model_id: stage_config.model_id.clone(),
                    prompt: current_input,
                    max_tokens: Some(2048),
                    temperature: Some(0.7),
                    stream: true,
                    tools: None,
                };

                let mut stream = backend.generate_stream(request).await?;

                while let Some(chunk) = stream.next().await {
                    yield chunk?;
                }
            }
        }))
    }

    pub async fn get_metrics(&self) -> PipelineMetrics {
        self.metrics.read().await.clone()
    }
}

pub struct HybridPipeline {
    local_pipeline: InferencePipeline,
    cloud_pipeline: InferencePipeline,
    fallback_enabled: bool,
}

impl HybridPipeline {
    pub fn new(
        local_config: PipelineConfig,
        cloud_config: PipelineConfig,
        fallback_enabled: bool,
    ) -> Self {
        Self {
            local_pipeline: InferencePipeline::new(local_config),
            cloud_pipeline: InferencePipeline::new(cloud_config),
            fallback_enabled,
        }
    }

    pub fn register_local_backend(&mut self, name: String, backend: Arc<dyn InferenceBackend>) {
        self.local_pipeline.register_backend(name, backend);
    }

    pub fn register_cloud_backend(&mut self, name: String, backend: Arc<dyn InferenceBackend>) {
        self.cloud_pipeline.register_backend(name, backend);
    }

    pub async fn execute(&self, input: String) -> InferenceResult<String> {
        let local_result = self.local_pipeline.execute(input.clone()).await;

        match local_result {
            Ok(result) => Ok(result),
            Err(e) if self.fallback_enabled => {
                self.cloud_pipeline.execute(input).await
            }
            Err(e) => Err(e),
        }
    }

    pub async fn get_local_metrics(&self) -> PipelineMetrics {
        self.local_pipeline.get_metrics().await
    }

    pub async fn get_cloud_metrics(&self) -> PipelineMetrics {
        self.cloud_pipeline.get_metrics().await
    }
}
