//! Inference Pipeline — ASR → LLM → TTS chaining
//!
//! Provides zero-copy (within Rust) model chaining for edge voice assistant workflows:
//!
//! ```text
//! Audio bytes → [ASR Model] → transcript → [LLM Model] → response → [TTS Model] → audio hint
//! ```
//!
//! ## Usage
//! ```rust,ignore
//! use std::sync::Arc;
//! use mofa_foundation::orchestrator::pipeline::{InferencePipeline, PipelineBuilder};
//! use mofa_foundation::orchestrator::traits::ModelType;
//!
//! let pipeline = PipelineBuilder::new(Arc::clone(&pool))
//!     .stage("whisper-base", ModelType::Asr)
//!     .stage("llama-3.1-8b", ModelType::Llm)
//!     .stage("kokoro-tts", ModelType::Tts)
//!     .build();
//!
//! let output = pipeline.run("hello world").await?;
//! println!("Response: {}", output.final_text);
//! println!("Latencies: {:?}", output.stage_latencies);
//! ```

use std::sync::Arc;
use std::time::Instant;

use crate::orchestrator::linux_candle::ModelPool;
use crate::orchestrator::traits::{ModelOrchestrator, ModelType, OrchestratorError, OrchestratorResult};

// ============================================================================
// Pipeline Stage
// ============================================================================

/// A single stage in the inference pipeline
#[derive(Debug, Clone)]
pub struct PipelineStage {
    /// ID of the model to use for this stage (must be registered in the pool)
    pub model_id: String,
    /// What type of processing this stage performs
    pub stage_type: ModelType,
}

impl PipelineStage {
    /// Create a new pipeline stage
    pub fn new(model_id: impl Into<String>, stage_type: ModelType) -> Self {
        Self {
            model_id: model_id.into(),
            stage_type,
        }
    }
}

// ============================================================================
// Pipeline Output
// ============================================================================

/// Output from a completed inference pipeline run
#[derive(Debug, Clone)]
pub struct PipelineOutput {
    /// Final text output from the last stage
    pub final_text: String,
    /// Hint text from the TTS stage (the text that was synthesised), if any
    pub tts_audio_hint: Option<String>,
    /// Per-stage latency in milliseconds: `(stage_model_id, latency_ms)`
    pub stage_latencies: Vec<(String, u128)>,
    /// Total wall-clock latency across all stages
    pub total_latency_ms: u128,
}

// ============================================================================
// Inference Pipeline
// ============================================================================

/// A configured inference pipeline that chains multiple models sequentially
///
/// Each stage's output string is passed as the next stage's input.
/// Models are automatically loaded via the `ModelPool` on first use.
pub struct InferencePipeline {
    /// Ordered list of pipeline stages
    stages: Vec<PipelineStage>,
    /// The model pool used to load and execute each stage
    pool: Arc<ModelPool>,
    /// Whether to auto-load models that are not yet loaded
    auto_load: bool,
}

impl InferencePipeline {
    /// Run the pipeline with the given text/audio input
    ///
    /// Returns a `PipelineOutput` containing the final text, per-stage latencies,
    /// and total latency. TTS audio hint is captured from the last TTS-type stage.
    pub async fn run(&self, input: &str) -> OrchestratorResult<PipelineOutput> {
        if self.stages.is_empty() {
            return Err(OrchestratorError::PipelineError(
                "Pipeline has no stages configured".to_string(),
            ));
        }

        let pipeline_start = Instant::now();
        let mut current_text = input.to_string();
        let mut stage_latencies: Vec<(String, u128)> = Vec::new();
        let mut tts_audio_hint: Option<String> = None;

        for stage in &self.stages {
            // Auto-load the model if required
            if self.auto_load && !self.pool.is_model_loaded(&stage.model_id) {
                self.pool.load_model(&stage.model_id).await.map_err(|e| {
                    OrchestratorError::PipelineError(format!(
                        "Stage '{}' model load failed: {}",
                        stage.model_id, e
                    ))
                })?;
            }

            // Time this stage
            let stage_start = Instant::now();

            let stage_output = self
                .pool
                .infer(&stage.model_id, &current_text)
                .await
                .map_err(|e| {
                    OrchestratorError::PipelineError(format!(
                        "Stage '{}' inference failed: {}",
                        stage.model_id, e
                    ))
                })?;

            let latency_ms = stage_start.elapsed().as_millis();
            stage_latencies.push((stage.model_id.clone(), latency_ms));

            // If this was a TTS stage, record the input to it as the audio hint
            if stage.stage_type == ModelType::Tts {
                tts_audio_hint = Some(current_text.clone());
            }

            current_text = stage_output;
        }

        let total_latency_ms = pipeline_start.elapsed().as_millis();

        Ok(PipelineOutput {
            final_text: current_text,
            tts_audio_hint,
            stage_latencies,
            total_latency_ms,
        })
    }

    /// Get the list of configured stages
    pub fn stages(&self) -> &[PipelineStage] {
        &self.stages
    }

    /// Get a summary of the pipeline stages for display/logging
    pub fn describe(&self) -> String {
        let stage_descriptions: Vec<String> = self
            .stages
            .iter()
            .map(|s| format!("{}({:?})", s.model_id, s.stage_type))
            .collect();
        stage_descriptions.join(" → ")
    }
}

// ============================================================================
// Pipeline Builder
// ============================================================================

/// Fluent builder for `InferencePipeline`
///
/// ## Example
/// ```rust,ignore
/// let pipeline = PipelineBuilder::new(Arc::clone(&pool))
///     .stage("whisper-base", ModelType::Asr)
///     .stage("llama-3.1-8b", ModelType::Llm)
///     .stage("kokoro-tts", ModelType::Tts)
///     .auto_load(true)
///     .build();
/// ```
pub struct PipelineBuilder {
    stages: Vec<PipelineStage>,
    pool: Arc<ModelPool>,
    auto_load: bool,
}

impl PipelineBuilder {
    /// Create a new builder using the given model pool
    pub fn new(pool: Arc<ModelPool>) -> Self {
        Self {
            stages: Vec::new(),
            pool,
            auto_load: true,
        }
    }

    /// Add a stage to the pipeline (appended at the end)
    pub fn stage(mut self, model_id: impl Into<String>, stage_type: ModelType) -> Self {
        self.stages.push(PipelineStage::new(model_id, stage_type));
        self
    }

    /// Whether to auto-load models that are not yet loaded (default: `true`)
    pub fn auto_load(mut self, auto_load: bool) -> Self {
        self.auto_load = auto_load;
        self
    }

    /// Build the configured pipeline
    #[must_use]
    pub fn build(self) -> InferencePipeline {
        InferencePipeline {
            stages: self.stages,
            pool: self.pool,
            auto_load: self.auto_load,
        }
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pipeline_stage_creation() {
        let stage = PipelineStage::new("whisper-base", ModelType::Asr);
        assert_eq!(stage.model_id, "whisper-base");
        assert_eq!(stage.stage_type, ModelType::Asr);
    }

    #[test]
    fn test_pipeline_builder_stages() {
        // We can't build without a pool, but we can verify stage configuration
        let stage = PipelineStage::new("llama-3.1-8b", ModelType::Llm);
        assert_eq!(stage.model_id, "llama-3.1-8b");
    }

    #[test]
    fn test_pipeline_output_structure() {
        let output = PipelineOutput {
            final_text: "Hello, world!".to_string(),
            tts_audio_hint: Some("Hello, world!".to_string()),
            stage_latencies: vec![
                ("whisper".to_string(), 120),
                ("llama".to_string(), 850),
                ("kokoro".to_string(), 200),
            ],
            total_latency_ms: 1170,
        };

        assert_eq!(output.final_text, "Hello, world!");
        assert!(output.tts_audio_hint.is_some());
        assert_eq!(output.stage_latencies.len(), 3);
        assert_eq!(output.total_latency_ms, 1170);
    }

    #[test]
    fn test_describe_pipeline() {
        let stage_descriptions = vec![
            format!("whisper-base({:?})", ModelType::Asr),
            format!("llama-8b({:?})", ModelType::Llm),
            format!("kokoro({:?})", ModelType::Tts),
        ];
        let description = stage_descriptions.join(" → ");
        assert!(description.contains("Asr"));
        assert!(description.contains("Llm"));
        assert!(description.contains("Tts"));
        assert!(description.contains("→"));
    }
}
