//! Model Orchestrator Traits
//!
//! This module defines the core traits for managing and orchestrating edge ML models:
//! - `ModelProvider`: Trait for loading, querying, and managing a single model instance
//! - `ModelOrchestrator`: Trait for orchestrating multiple models with lifecycle and scheduling

use async_trait::async_trait;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use thiserror::Error;

// ============================================================================
// Error Types
// ============================================================================

/// Model orchestrator error types
#[derive(Debug, Clone, Error)]
pub enum OrchestratorError {
    /// Model loading failed
    #[error("Model load failed: {0}")]
    ModelLoadFailed(String),

    /// Model inference failed
    #[error("Model inference failed: {0}")]
    InferenceFailed(String),

    /// Model not found
    #[error("Model not found: {0}")]
    ModelNotFound(String),

    /// Memory constrained - insufficient available system memory
    #[error("Memory constrained: {0}")]
    MemoryConstrained(String),

    /// Device error (GPU unavailable, etc.)
    #[error("Device error: {0}")]
    DeviceError(String),

    /// Configuration error
    #[error("Configuration error: {0}")]
    ConfigError(String),

    /// Pool is at capacity and cannot accept more models
    #[error("Pool at capacity: {0}")]
    PoolCapacityExceeded(String),

    /// LRU eviction failed
    #[error("LRU eviction failed: {0}")]
    EvictionFailed(String),

    /// Serialization error
    #[error("Serialization error: {0}")]
    SerializationError(String),

    /// Other errors
    #[error("Orchestrator error: {0}")]
    Other(String),
}

/// Result type for orchestrator operations
pub type OrchestratorResult<T> = Result<T, OrchestratorError>;

// ============================================================================
// Model Provider Trait
// ============================================================================

/// Configuration for a model provider
#[derive(Debug, Clone)]
pub struct ModelProviderConfig {
    /// Name of the model
    pub model_name: String,
    /// Model path or identifier
    pub model_path: String,
    /// Device type ("cuda" or "cpu")
    pub device: String,
    /// Maximum context window
    pub max_context_length: Option<usize>,
    /// Quantization level (e.g., "q4_0", "q8_0", "f32")
    pub quantization: Option<String>,
    /// Custom configuration options
    pub extra_config: HashMap<String, Value>,
}

/// Model provider trait - handles loading and inference for a single model
///
/// Implementers are responsible for:
/// - Loading models in their preferred format (GGUF, safetensors, etc.)
/// - Managing model inference
/// - Reporting memory usage
/// - Graceful cleanup/unloading
#[async_trait]
pub trait ModelProvider: Send + Sync {
    /// Get the provider name
    fn name(&self) -> &str;

    /// Get the model identifier/name
    fn model_id(&self) -> &str;

    /// Load the model into memory
    ///
    /// # Errors
    /// - `ModelLoadFailed`: If model loading encounters an error
    /// - `DeviceError`: If the specified device is unavailable
    async fn load(&mut self) -> OrchestratorResult<()>;

    /// Unload the model from memory, freeing up resources
    async fn unload(&mut self) -> OrchestratorResult<()>;

    /// Check if the model is currently loaded
    fn is_loaded(&self) -> bool;

    /// Run inference on the model with the given prompt
    ///
    /// # Arguments
    /// * `input` - The input prompt or query
    ///
    /// # Returns
    /// Model response as a string
    async fn infer(&self, input: &str) -> OrchestratorResult<String>;

    /// Get the current memory usage in bytes
    fn memory_usage_bytes(&self) -> u64;

    /// Get metadata about the model
    fn get_metadata(&self) -> HashMap<String, Value>;

    /// Health check - verify the model is functioning correctly
    async fn health_check(&self) -> OrchestratorResult<bool>;
}

// ============================================================================
// Model Orchestrator Trait
// ============================================================================

/// Statistics about the current model pool
#[derive(Debug, Clone)]
pub struct PoolStatistics {
    /// Number of currently loaded models
    pub loaded_models_count: usize,
    /// Total memory used by all loaded models (bytes)
    pub total_memory_usage: u64,
    /// Available system memory (bytes)
    pub available_memory: u64,
    /// Number of models queued for loading
    pub queued_models_count: usize,
    /// Timestamp of statistics collection
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

/// Model orchestrator trait - manages multiple models with lifecycle and scheduling
///
/// Implementers are responsible for:
/// - Managing a pool of models with LRU eviction
/// - Monitoring memory usage and enforcing admission control
/// - Scheduling model loads/unloads based on demand
/// - Handling concurrent inference requests safely
#[async_trait]
pub trait ModelOrchestrator: Send + Sync {
    /// Get the orchestrator name
    fn name(&self) -> &str;

    /// Register a model in the orchestrator
    ///
    /// # Arguments
    /// * `config` - Configuration for the model to register
    ///
    /// # Errors
    /// - `ConfigError`: If the configuration is invalid
    /// - `PoolCapacityExceeded`: If the pool is at maximum capacity
    async fn register_model(&self, config: ModelProviderConfig) -> OrchestratorResult<()>;

    /// Unregister a model from the orchestrator
    async fn unregister_model(&self, model_id: &str) -> OrchestratorResult<()>;

    /// Load a model into memory for inference
    ///
    /// Automatically handles:
    /// - Memory pressure: Triggers LRU eviction if memory is constrained
    /// - Concurrent access: Ensures thread-safe loading
    ///
    /// # Arguments
    /// * `model_id` - The identifier of the model to load
    ///
    /// # Errors
    /// - `ModelNotFound`: If the model isn't registered
    /// - `MemoryConstrained`: If memory is too low even after eviction
    /// - `ModelLoadFailed`: If loading encounters an error
    async fn load_model(&self, model_id: &str) -> OrchestratorResult<()>;

    /// Unload a model from memory
    async fn unload_model(&self, model_id: &str) -> OrchestratorResult<()>;

    /// Check if a model is loaded
    fn is_model_loaded(&self, model_id: &str) -> bool;

    /// Run inference on a model
    ///
    /// Automatically:
    /// - Loads the model if not already loaded
    /// - Updates LRU access time
    /// - Manages memory pressure
    ///
    /// # Arguments
    /// * `model_id` - The model to run inference on
    /// * `input` - The input prompt/query
    async fn infer(&self, model_id: &str, input: &str) -> OrchestratorResult<String>;

    /// Get current pool statistics
    fn get_statistics(&self) -> OrchestratorResult<PoolStatistics>;

    /// Get list of registered model IDs
    fn list_models(&self) -> Vec<String>;

    /// Get list of currently loaded model IDs
    fn list_loaded_models(&self) -> Vec<String>;

    /// Manually trigger LRU-based eviction to free the specified amount of memory
    ///
    /// # Arguments
    /// * `target_bytes` - Amount of memory to free (bytes)
    ///
    /// # Returns
    /// Number of models evicted
    async fn trigger_eviction(&self, target_bytes: u64) -> OrchestratorResult<usize>;

    /// Set the maximum memory threshold (bytes)
    /// When exceeded, automatic eviction is triggered
    async fn set_memory_threshold(&self, bytes: u64) -> OrchestratorResult<()>;

    /// Get the current memory threshold (bytes)
    fn get_memory_threshold(&self) -> u64;

    /// Set the model idle timeout (seconds)
    /// Models unused for longer than this are candidates for eviction
    async fn set_idle_timeout_secs(&self, secs: u64) -> OrchestratorResult<()>;

    /// Get the current idle timeout in seconds
    fn get_idle_timeout_secs(&self) -> u64;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_display() {
        let err = OrchestratorError::ModelNotFound("gpt2".to_string());
        assert_eq!(err.to_string(), "Model not found: gpt2");

        let err = OrchestratorError::MemoryConstrained("Need 8GB, have 2GB".to_string());
        assert!(err.to_string().contains("Memory constrained"));
    }

    #[test]
    fn test_config_creation() {
        let config = ModelProviderConfig {
            model_name: "Llama-2-7B".to_string(),
            model_path: "/models/llama-2-7b.gguf".to_string(),
            device: "cuda".to_string(),
            max_context_length: Some(4096),
            quantization: Some("q4_0".to_string()),
            extra_config: HashMap::new(),
        };

        assert_eq!(config.model_name, "Llama-2-7B");
        assert_eq!(config.device, "cuda");
    }
}
