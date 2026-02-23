//! Pluggable Inference Backend Abstraction
//!
//! This module defines the `InferenceBackend` trait, which provides a
//! hardware-agnostic interface for model inference. Any backend (e.g.,
//! OminiX-MLX on macOS, Candle on Windows/Linux, ONNX Runtime) can
//! implement this trait independently.
//!
//! The orchestrator uses detected hardware capabilities to select the
//! appropriate backend at runtime.

use anyhow::Result;
use std::collections::HashMap;

/// Configuration for loading a model into an inference backend.
#[derive(Debug, Clone)]
pub struct ModelConfig {
    /// Unique identifier for the model (e.g., "qwen-7b", "funasr-base").
    pub model_id: String,
    /// File system path or HuggingFace repo identifier for the model weights.
    pub model_path: String,
    /// Task type this model is intended for.
    pub task: TaskType,
    /// Quantization level for the model weights.
    pub quantization: Quantization,
    /// Optional key-value metadata (e.g., max sequence length, vocab path).
    pub metadata: HashMap<String, String>,
}

/// The type of task a model is designed to perform.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TaskType {
    /// Automatic Speech Recognition (e.g., FunASR, Whisper).
    Asr,
    /// Large Language Model text generation (e.g., Qwen, Llama).
    Llm,
    /// Text-to-Speech synthesis (e.g., GPT-SoVITS, Kokoro).
    Tts,
    /// Text/image embedding generation.
    Embedding,
    /// A custom or unrecognized task type.
    Other(String),
}

/// Quantization level for model weights, affecting memory usage and quality.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Quantization {
    /// Full precision (32-bit floating point).
    F32,
    /// Half precision (16-bit floating point).
    F16,
    /// 8-bit integer quantization.
    Int8,
    /// 4-bit integer quantization (lowest memory, reduced quality).
    Int4,
}

/// Input payload for an inference request.
#[derive(Debug, Clone)]
pub struct InferenceRequest {
    /// The model to run inference on (by model_id).
    pub model_id: String,
    /// Input data — a text prompt, audio bytes, or other input depending on TaskType.
    pub input: InferenceInput,
    /// Maximum number of tokens to generate (applicable for LLM tasks).
    pub max_tokens: Option<u32>,
    /// Sampling temperature (applicable for LLM tasks).
    pub temperature: Option<f32>,
}

/// The input data for an inference request, varying by task type.
#[derive(Debug, Clone)]
pub enum InferenceInput {
    /// Text input (for LLM, TTS, Embedding tasks).
    Text(String),
    /// Raw audio bytes (for ASR tasks).
    Audio(Vec<u8>),
}

/// Output payload from an inference request.
#[derive(Debug, Clone)]
pub struct InferenceResponse {
    /// The model that produced this response.
    pub model_id: String,
    /// The generated output.
    pub output: InferenceOutput,
}

/// The output data from an inference request, varying by task type.
#[derive(Debug, Clone)]
pub enum InferenceOutput {
    /// Generated text (from LLM or ASR tasks).
    Text(String),
    /// Generated audio bytes (from TTS tasks).
    Audio(Vec<u8>),
    /// Embedding vector (from Embedding tasks).
    Embedding(Vec<f32>),
}

/// A pluggable inference backend that can load models and run inference.
///
/// Implementations of this trait provide the actual model execution logic
/// for a specific hardware/software stack (e.g., MLX on Apple Silicon,
/// Candle on CUDA, ONNX Runtime for cross-platform).
///
/// # Example
///
/// ```rust,ignore
/// struct MlxBackend { /* ... */ }
///
/// impl InferenceBackend for MlxBackend {
///     fn name(&self) -> &str { "mlx" }
///     fn load_model(&mut self, config: &ModelConfig) -> Result<()> { /* ... */ }
///     fn generate(&self, request: &InferenceRequest) -> Result<InferenceResponse> { /* ... */ }
///     fn unload_model(&mut self, model_id: &str) -> Result<()> { /* ... */ }
///     fn is_model_loaded(&self, model_id: &str) -> bool { /* ... */ }
/// }
/// ```
pub trait InferenceBackend: Send + Sync {
    /// Returns the name of this backend (e.g., "mlx", "candle", "onnx").
    fn name(&self) -> &str;

    /// Load a model into this backend using the provided configuration.
    fn load_model(&mut self, config: &ModelConfig) -> Result<()>;

    /// Run inference on a loaded model.
    fn generate(&self, request: &InferenceRequest) -> Result<InferenceResponse>;

    /// Unload a model from this backend, freeing its resources.
    fn unload_model(&mut self, model_id: &str) -> Result<()>;

    /// Check whether a model is currently loaded in this backend.
    fn is_model_loaded(&self, model_id: &str) -> bool;
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A minimal mock backend for testing the trait contract.
    struct MockBackend {
        loaded: std::collections::HashSet<String>,
    }

    impl MockBackend {
        fn new() -> Self {
            Self {
                loaded: std::collections::HashSet::new(),
            }
        }
    }

    impl InferenceBackend for MockBackend {
        fn name(&self) -> &str {
            "mock"
        }

        fn load_model(&mut self, config: &ModelConfig) -> Result<()> {
            self.loaded.insert(config.model_id.clone());
            Ok(())
        }

        fn generate(&self, request: &InferenceRequest) -> Result<InferenceResponse> {
            if !self.is_model_loaded(&request.model_id) {
                return Err(anyhow::anyhow!("Model {} is not loaded.", request.model_id));
            }
            Ok(InferenceResponse {
                model_id: request.model_id.clone(),
                output: InferenceOutput::Text("Mock output.".to_string()),
            })
        }

        fn unload_model(&mut self, model_id: &str) -> Result<()> {
            self.loaded.remove(model_id);
            Ok(())
        }

        fn is_model_loaded(&self, model_id: &str) -> bool {
            self.loaded.contains(model_id)
        }
    }

    #[test]
    fn test_mock_backend_lifecycle() {
        let mut backend = MockBackend::new();

        assert_eq!(backend.name(), "mock");
        assert!(!backend.is_model_loaded("test-model"));

        // Load a model
        let config = ModelConfig {
            model_id: "test-model".to_string(),
            model_path: "/path/to/model".to_string(),
            task: TaskType::Llm,
            quantization: Quantization::Int8,
            metadata: HashMap::new(),
        };
        backend.load_model(&config).unwrap();
        assert!(backend.is_model_loaded("test-model"));

        // Run inference
        let request = InferenceRequest {
            model_id: "test-model".to_string(),
            input: InferenceInput::Text("Hello".to_string()),
            max_tokens: Some(100),
            temperature: Some(0.7),
        };
        let response = backend.generate(&request).unwrap();
        assert_eq!(response.model_id, "test-model");

        // Unload
        backend.unload_model("test-model").unwrap();
        assert!(!backend.is_model_loaded("test-model"));
    }

    #[test]
    fn test_generate_fails_when_model_not_loaded() {
        let backend = MockBackend::new();
        let request = InferenceRequest {
            model_id: "missing-model".to_string(),
            input: InferenceInput::Text("Hello".to_string()),
            max_tokens: None,
            temperature: None,
        };
        assert!(backend.generate(&request).is_err());
    }
}
