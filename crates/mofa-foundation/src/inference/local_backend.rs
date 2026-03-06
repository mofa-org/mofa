//! Local Inference Backend Abstraction
//!
//! This module defines the `LocalInferenceBackend` trait that provides a unified
//! interface for local inference engines (e.g., Candle, llama.cpp, MLX).
//!
//! The trait enables the inference orchestrator to work with different local
//! inference backends through a consistent API, supporting:
//! - Model lifecycle management (load/unload)
//! - Synchronous generation
//! - Streaming generation
//!
//! # Implementing the Trait
//!
//! New local inference engines can implement this trait:
//!
//! ```ignore
//! impl LocalInferenceBackend for MyLocalEngine {
//!     fn load_model(&self, model_id: &str) -> Result<()> { ... }
//!     fn generate(&self, request: InferenceRequest) -> Result<InferenceResponse> { ... }
//!     fn generate_stream(&self, request: InferenceRequest) -> Result<impl Stream<Item = InferenceChunk>> { ... }
//!     fn unload_model(&self, model_id: &str) -> Result<()> { ... }
//! }
//! ```

use crate::inference::types::{InferenceRequest, Precision};
use crate::orchestrator::OrchestratorError;
use futures::Stream;
use std::fmt;

/// Result type for local backend operations
pub type BackendResult<T> = Result<T, OrchestratorError>;

/// A single chunk in a streaming generation response
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct InferenceChunk {
    /// The text content of this chunk
    pub text: String,
    /// Whether this is the final chunk
    pub is_final: bool,
    /// Optional token count for this chunk
    #[serde(skip_serializing_if = "Option::is_none")]
    pub token_count: Option<usize>,
}

impl InferenceChunk {
    /// Create a new inference chunk
    pub fn new(text: impl Into<String>, is_final: bool) -> Self {
        Self {
            text: text.into(),
            is_final,
            token_count: None,
        }
    }

    /// Create a new chunk with token count
    pub fn with_tokens(text: impl Into<String>, is_final: bool, tokens: usize) -> Self {
        Self {
            text: text.into(),
            is_final,
            token_count: Some(tokens),
        }
    }
}

impl fmt::Display for InferenceChunk {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.text)
    }
}

/// The response from a local inference backend
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct InferenceResponse {
    /// The generated output text
    pub output: String,
    /// The model ID used for inference
    pub model_id: String,
    /// The actual precision used
    pub actual_precision: Precision,
    /// Total tokens generated
    pub total_tokens: usize,
    /// Whether the response was truncated
    pub truncated: bool,
}

impl InferenceResponse {
    /// Create a new inference response
    pub fn new(
        output: impl Into<String>,
        model_id: impl Into<String>,
        actual_precision: Precision,
        total_tokens: usize,
    ) -> Self {
        Self {
            output: output.into(),
            model_id: model_id.into(),
            actual_precision,
            total_tokens,
            truncated: false,
        }
    }

    /// Create a truncated response
    pub fn truncated(
        output: impl Into<String>,
        model_id: impl Into<String>,
        actual_precision: Precision,
        total_tokens: usize,
    ) -> Self {
        Self {
            output: output.into(),
            model_id: model_id.into(),
            actual_precision,
            total_tokens,
            truncated: true,
        }
    }
}

/// Unified trait for local inference backends.
///
/// This trait defines the interface that local inference engines
/// (e.g., LinuxCandleProvider, llama.cpp adapter, MLX adapter) must implement
/// to work with the MoFA inference orchestrator.
///
/// Implementers must be thread-safe (`Send + Sync`) to allow concurrent
/// access from multiple tasks.
pub trait LocalInferenceBackend: Send + Sync {
    /// Load a model into memory.
    ///
    /// # Arguments
    /// * `model_id` - The identifier of the model to load
    ///
    /// # Errors
    /// Returns an error if:
    /// - The model cannot be found
    /// - Loading fails due to insufficient memory
    /// - The model format is invalid
    fn load_model(&self, model_id: &str) -> BackendResult<()>;

    /// Generate text from a prompt (synchronous).
    ///
    /// # Arguments
    /// * `request` - The inference request containing prompt and parameters
    ///
    /// # Errors
    /// Returns an error if:
    /// - The model is not loaded
    /// - Inference fails
    /// - The model ID in the request doesn't match a loaded model
    fn generate(&self, request: InferenceRequest) -> BackendResult<InferenceResponse>;

    /// Generate text from a prompt with streaming responses.
    ///
    /// Returns a stream of text chunks that can be consumed incrementally.
    ///
    /// # Arguments
    /// * `request` - The inference request containing prompt and parameters
    ///
    /// # Errors
    /// Returns an error if:
    /// - The model is not loaded
    /// - Inference fails
    fn generate_stream(
        &self,
        request: InferenceRequest,
    ) -> BackendResult<Box<dyn Stream<Item = Result<InferenceChunk, OrchestratorError>> + Send>>;

    /// Unload a model from memory.
    ///
    /// # Arguments
    /// * `model_id` - The identifier of the model to unload
    ///
    /// # Errors
    /// Returns an error if:
    /// - The model is not currently loaded
    /// - Unloading fails
    fn unload_model(&self, model_id: &str) -> BackendResult<()>;
}

#[cfg(test)]
mod tests {
    #[cfg(test)]
    use super::*;

    #[test]
    fn test_inference_chunk_creation() {
        let chunk = InferenceChunk::new("Hello", false);
        assert_eq!(chunk.text, "Hello");
        assert!(!chunk.is_final);
        assert!(chunk.token_count.is_none());
    }

    #[test]
    fn test_inference_chunk_with_tokens() {
        let chunk = InferenceChunk::with_tokens("World", true, 5);
        assert_eq!(chunk.text, "World");
        assert!(chunk.is_final);
        assert_eq!(chunk.token_count, Some(5));
    }

    #[test]
    fn test_inference_response_creation() {
        let response = InferenceResponse::new("test output", "llama-3", Precision::F16, 100);
        assert_eq!(response.output, "test output");
        assert_eq!(response.model_id, "llama-3");
        assert_eq!(response.actual_precision, Precision::F16);
        assert_eq!(response.total_tokens, 100);
        assert!(!response.truncated);
    }

    #[test]
    fn test_inference_response_truncated() {
        let response = InferenceResponse::truncated("truncated output", "llama-3", Precision::Q4, 50);
        assert!(response.truncated);
    }
}
