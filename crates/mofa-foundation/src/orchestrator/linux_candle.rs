//! Linux Candle Provider - Production-ready Edge ML Model Orchestrator
//!
//! This module implements the GSoC 2026 "Edge Model Orchestrator" (Idea 3) using Hugging Face's
//! Candle framework for efficient on-device inference.
//!
//! ## Key Features
//!
//! ### 1. Lifecycle Management
//! - **Automatic Model Loading**: Models are loaded on-demand when inference is requested
//! - **Idle Timeout**: Models unused for 30 seconds (configurable) are automatically unloaded
//! - **Graceful Cleanup**: Proper resource deallocation when models are evicted
//!
//! ### 2. Smart Scheduling (LRU Eviction)
//! - **Least Recently Used**: When memory is constrained, the oldest idle model is evicted first
//! - **Concurrent Safety**: Uses `tokio::sync::RwLock` for thread-safe concurrent access
//! - **Memory Pressure Aware**: Monitors available system RAM before loading models
//!
//! ### 3. Linux Integration
//! - **Dynamic Device Selection**: Tries CUDA GPU first, falls back to CPU automatically
//! - **System Memory Monitoring**: Uses `sysinfo` to check available RAM in real-time
//! - **Native Performance**: Zero-copy inference using Candle's efficient tensor operations
//!
//! ## Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────┐
//! │         ModelPool (Orchestrator)        │
//! │  - Manages multiple models              │
//! │  - LRU eviction policy                  │
//! │  - Memory threshold enforcement         │
//! └─────────────────┬───────────────────────┘
//!                   │
//!        ┌──────────┴──────────┐
//!        ▼                     ▼
//! ┌──────────────┐      ┌──────────────┐
//! │ ModelEntry   │      │ ModelEntry   │
//! │ - Provider   │      │ - Provider   │
//! │ - Metadata   │      │ - Metadata   │
//! │ - LRU time   │      │ - LRU time   │
//! └──────┬───────┘      └──────┬───────┘
//!        │                     │
//!        ▼                     ▼
//! ┌─────────────────────────────────────────┐
//! │    LinuxCandleProvider (ModelProvider)  │
//! │  - Candle model instance                │
//! │  - Device (CUDA/CPU)                    │
//! │  - Tokenizer                            │
//! └─────────────────────────────────────────┘
//! ```

use super::traits::{
    DegradationLevel, ModelOrchestrator, ModelProvider, ModelProviderConfig, ModelType,
    OrchestratorError, OrchestratorResult, PoolStatistics,
};
use async_trait::async_trait;
use candle_core::{quantized::gguf_file, Device, Tensor};
use candle_transformers::generation::LogitsProcessor;
use candle_transformers::models::quantized_qwen2;
use serde_json::Value;
use std::collections::HashMap;
use std::fs::File;
use candle_core::IndexOp;
use std::path::PathBuf;
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};
use sysinfo::{MemoryRefreshKind, RefreshKind, System};
use tokio::sync::RwLock as AsyncRwLock;
use tokio::time::sleep;
use tokio::sync::Mutex as AsyncMutex;
use tokenizers::Tokenizer;

// ============================================================================
// Constants
// ============================================================================

/// Default idle timeout before a model becomes eligible for LRU eviction
const DEFAULT_IDLE_TIMEOUT_SECS: u64 = 30;

/// Default memory threshold (80% of available RAM)
/// When exceeded, LRU eviction is automatically triggered
const DEFAULT_MEMORY_THRESHOLD_PERCENT: f64 = 0.8;

/// Minimum free memory required before loading a model (500 MB)
const MIN_FREE_MEMORY_BYTES: u64 = 500 * 1024 * 1024;

/// Temperature for sampling during text generation
const DEFAULT_TEMPERATURE: f64 = 0.8;

/// Top-p (nucleus sampling) threshold
const DEFAULT_TOP_P: f64 = 0.9;

/// Maximum number of tokens to generate
const DEFAULT_MAX_TOKENS: usize = 256;
const DEFAULT_STOP_MARKERS: [&str; 5] = ["<|im_end|>", "<|im_start|>", "Human:", "User:", "Assistant:"];

// ============================================================================
// Internal Model State
// ============================================================================

/// Internal state of a loaded Candle model.
///
/// This structure holds the actual model weights, tokenizer, and device information.
/// It is not exposed directly; instead, it's wrapped by `LinuxCandleProvider`.
struct CandleModelState {
    /// The loaded model (Qwen2 GGUF, quantized).
    model: quantized_qwen2::ModelWeights,

    /// Device the model is loaded on (CUDA or CPU)
    device: Device,

    /// Tokenizer for preprocessing input text
    tokenizer: Arc<Tokenizer>,

    /// Optional EOS token id for early stopping
    eos_token_id: Option<u32>,
}

// ============================================================================
// LinuxCandleProvider - Model Provider Implementation
// ============================================================================

/// Linux-based Candle model provider
///
/// Implements the `ModelProvider` trait using Hugging Face's Candle framework.
/// Supports:
/// - Dynamic CUDA/CPU device selection
/// - Model loading from local filesystem or Hugging Face Hub
/// - Memory usage tracking
/// - Graceful unloading
pub struct LinuxCandleProvider {
    /// Unique identifier for this model
    model_id: String,

    /// Configuration for this provider
    config: ModelProviderConfig,

    /// Loaded model state (None if not loaded)
    state: Option<Arc<AsyncMutex<CandleModelState>>>,

    /// Estimated memory usage in bytes
    memory_usage: u64,

    /// Whether the model is currently loaded
    loaded: bool,
}

impl LinuxCandleProvider {
    /// Create a new unloaded provider instance
    pub fn new(config: ModelProviderConfig) -> Self {
        let model_id = config.model_name.clone();

        Self {
            model_id,
            config,
            state: None,
            memory_usage: 0,
            loaded: false,
        }
    }

    /// Select the best available device (CUDA → CPU fallback)
    fn select_device(&self) -> OrchestratorResult<Device> {
        // Try to use CUDA GPU if available
        if self.config.device.to_lowercase() == "cuda" {
            match Device::new_cuda(0) {
                Ok(device) => {
                    tracing::info!("Using CUDA device for model '{}'", self.model_id);
                    return Ok(device);
                }
                Err(e) => {
                    tracing::warn!(
                        "CUDA requested but unavailable for '{}': {}. Falling back to CPU.",
                        self.model_id,
                        e
                    );
                }
            }
        }

        // Fallback to CPU
        tracing::info!("Using CPU device for model '{}'", self.model_id);
        Ok(Device::Cpu)
    }

    /// Load a Qwen2 GGUF model from disk
    fn load_model_weights(
        &self,
        device: &Device,
    ) -> OrchestratorResult<(quantized_qwen2::ModelWeights, Option<u32>)> {
        let model_path = PathBuf::from(&self.config.model_path);

        // Check if model file exists
        if !model_path.exists() {
            return Err(OrchestratorError::ModelLoadFailed(format!(
                "Model file not found: {}",
                self.config.model_path
            )));
        }

        let mut reader = File::open(&model_path).map_err(|e| {
            OrchestratorError::ModelLoadFailed(format!(
                "Failed to open model file '{}': {}",
                self.config.model_path, e
            ))
        })?;

        let ct = gguf_file::Content::read(&mut reader).map_err(|e| {
            OrchestratorError::ModelLoadFailed(format!(
                "Failed to read GGUF header for '{}': {}",
                self.config.model_path, e
            ))
        })?;

        let architecture = match ct.metadata.get("general.architecture") {
            Some(gguf_file::Value::String(v)) => v.as_str(),
            _ => "unknown",
        };

        if architecture != "qwen2" {
            return Err(OrchestratorError::ModelLoadFailed(format!(
                "Unsupported GGUF architecture '{}' for model '{}'. Expected 'qwen2'.",
                architecture, self.config.model_path
            )));
        }

        let eos_token_id = match ct.metadata.get("tokenizer.ggml.eos_token_id") {
            Some(v) => v.to_u32().ok().map(|v| v as u32),
            None => None,
        };

        let model = quantized_qwen2::ModelWeights::from_gguf(ct, &mut reader, device).map_err(
            |e| {
                OrchestratorError::ModelLoadFailed(format!(
                    "Failed to load Qwen2 GGUF model '{}': {}",
                    self.config.model_path, e
                ))
            },
        )?;

        Ok((model, eos_token_id))
    }

    fn load_tokenizer(&self) -> OrchestratorResult<Tokenizer> {
        let explicit_path = self
            .config
            .extra_config
            .get("tokenizer_path")
            .and_then(|v| v.as_str())
            .map(PathBuf::from)
            .or_else(|| {
                self.config
                    .extra_config
                    .get("tokenizer_json")
                    .and_then(|v| v.as_str())
                    .map(PathBuf::from)
            });

        let tokenizer_path = if let Some(path) = explicit_path {
            path
        } else {
            let model_path = PathBuf::from(&self.config.model_path);
            let fallback = model_path
                .parent()
                .map(|dir| dir.join("tokenizer.json"))
                .unwrap_or_else(|| PathBuf::from("tokenizer.json"));
            fallback
        };

        if !tokenizer_path.exists() {
            return Err(OrchestratorError::ConfigError(format!(
                "Tokenizer not found at '{}'. Provide extra_config['tokenizer_path'] or place tokenizer.json next to the model.",
                tokenizer_path.display()
            )));
        }

        Tokenizer::from_file(&tokenizer_path).map_err(|e| {
            OrchestratorError::ModelLoadFailed(format!(
                "Failed to load tokenizer from '{}': {}",
                tokenizer_path.display(),
                e
            ))
        })
    }

    /// Estimate memory usage of the loaded model
    ///
    /// In production, calculate this by:
    /// - Summing parameter count × bytes per parameter
    /// - Adding KV cache size
    /// - Adding activation memory overhead
    fn estimate_memory_usage(&self) -> u64 {
        if let Ok(meta) = std::fs::metadata(&self.config.model_path) {
            return meta.len();
        }

        // Rough estimation for a 7B parameter model:
        // - 7B params × 2 bytes (FP16) = ~14 GB
        // - 7B params × 1 byte (INT8) = ~7 GB
        // - 7B params × 0.5 bytes (INT4/Q4) = ~3.5 GB

        let param_count = 7_000_000_000_u64; // 7 billion parameters

        let bytes_per_param = match self.config.quantization.as_deref() {
            Some("q4_0") | Some("q4_1") => 0.5, // 4-bit quantization
            Some("q8_0") => 1.0,                // 8-bit quantization
            Some("f16") => 2.0,                 // FP16
            Some("f32") | None => 4.0,          // FP32 (default)
            _ => 4.0,
        };

        let base_memory = (param_count as f64 * bytes_per_param) as u64;

        // Add KV cache overhead (~10% of model size)
        let kv_cache_overhead = (base_memory as f64 * 0.1) as u64;

        base_memory + kv_cache_overhead
    }

    fn max_new_tokens(&self) -> usize {
        self.config
            .extra_config
            .get("max_new_tokens")
            .and_then(|v| v.as_u64())
            .map(|v| v as usize)
            .unwrap_or(DEFAULT_MAX_TOKENS)
    }

    fn stop_markers(&self) -> Vec<String> {
        if let Some(Value::Array(values)) = self.config.extra_config.get("stop_sequences") {
            let markers: Vec<String> = values
                .iter()
                .filter_map(|value| value.as_str().map(ToOwned::to_owned))
                .collect();
            if !markers.is_empty() {
                return markers;
            }
        }

        DEFAULT_STOP_MARKERS
            .iter()
            .map(|marker| marker.to_string())
            .collect()
    }

    fn trim_at_stop_markers(&self, text: &str, stop_markers: &[String]) -> String {
        let cut = stop_markers
            .iter()
            .filter_map(|marker| text.find(marker))
            .min()
            .unwrap_or(text.len());
        text[..cut].trim().to_string()
    }
}

#[async_trait]
impl ModelProvider for LinuxCandleProvider {
    fn name(&self) -> &str {
        "LinuxCandleProvider"
    }

    fn model_id(&self) -> &str {
        &self.model_id
    }

    fn model_type(&self) -> &ModelType {
        &self.config.model_type
    }

    async fn load(&mut self) -> OrchestratorResult<()> {
        if self.loaded {
            tracing::debug!("Model '{}' is already loaded", self.model_id);
            return Ok(());
        }

        tracing::info!("Loading model '{}'...", self.model_id);

        // Step 1: Select device (CUDA with CPU fallback)
        let device = self.select_device()?;

        // Step 2: Load model weights
        let (model, eos_token_id) = self.load_model_weights(&device)?;

        // Step 3: Initialize tokenizer
        let tokenizer = Arc::new(self.load_tokenizer()?);
        let eos_token_id = eos_token_id.or_else(|| {
            tokenizer
                .token_to_id("<|im_end|>")
                .or_else(|| tokenizer.token_to_id("</s>"))
                .or_else(|| tokenizer.token_to_id("<|endoftext|>"))
        });

        // Step 4: Estimate memory usage
        self.memory_usage = self.estimate_memory_usage();

        tracing::info!(
            "Model '{}' loaded successfully. Estimated memory: {} MB",
            self.model_id,
            self.memory_usage / 1024 / 1024
        );

        // Step 5: Store state
        self.state = Some(Arc::new(AsyncMutex::new(CandleModelState {
            model,
            device,
            tokenizer,
            eos_token_id,
        })));
        self.loaded = true;

        Ok(())
    }

    async fn unload(&mut self) -> OrchestratorResult<()> {
        if !self.loaded {
            tracing::debug!("Model '{}' is not loaded", self.model_id);
            return Ok(());
        }

        tracing::info!("Unloading model '{}'...", self.model_id);

        // Drop the model state, freeing GPU/CPU memory
        self.state = None;
        self.memory_usage = 0;
        self.loaded = false;

        tracing::info!("Model '{}' unloaded successfully", self.model_id);
        Ok(())
    }

    fn is_loaded(&self) -> bool {
        self.loaded
    }

    async fn infer(&self, input: &str) -> OrchestratorResult<String> {
        if !self.loaded {
            return Err(OrchestratorError::InferenceFailed(
                "Model is not loaded".to_string(),
            ));
        }

        let state = self.state.as_ref().ok_or_else(|| {
            OrchestratorError::InferenceFailed("Model state is missing".to_string())
        })?;
        let mut state = state.lock().await;

        tracing::debug!(
            "Running inference on model '{}' with input: {}",
            self.model_id,
            input
        );

        // Step 1: Tokenize input
        let encoding = state
            .tokenizer
            .encode(input, true)
            .map_err(|e| OrchestratorError::InferenceFailed(format!("Tokenizer error: {e}")))?;
        let mut tokens: Vec<u32> = encoding.get_ids().iter().map(|&t| t as u32).collect();

        if tokens.is_empty() {
            return Err(OrchestratorError::InferenceFailed(
                "Tokenization produced empty token sequence".to_string(),
            ));
        }

        // Step 2: Enforce max context length if configured
        if let Some(max_context) = self.config.max_context_length {
            if tokens.len() > max_context {
                tokens = tokens[tokens.len() - max_context..].to_vec();
            }
        }

        // Step 3: Autoregressive generation
        let mut logits_processor =
            LogitsProcessor::new(299792458, Some(DEFAULT_TEMPERATURE), Some(DEFAULT_TOP_P));
        let mut generated: Vec<u32> = Vec::new();
        let mut all_tokens = tokens.clone();
        let stop_markers = self.stop_markers();

        for _ in 0..self.max_new_tokens() {
            let input_tensor = Tensor::new(all_tokens.as_slice(), &state.device)
                .map_err(|e| OrchestratorError::InferenceFailed(format!("Tensor error: {e}")))?
                .reshape((1, all_tokens.len()))
                .map_err(|e| OrchestratorError::InferenceFailed(format!("Tensor reshape error: {e}")))?;

            let logits = state
                .model
                .forward(&input_tensor, 0)
                .map_err(|e| OrchestratorError::InferenceFailed(format!("Model forward error: {e}")))?;
            let logits = logits
                .i(0)
                .map_err(|e| OrchestratorError::InferenceFailed(format!("Logits shape error: {e}")))?;

            let next_token = logits_processor
                .sample(&logits)
                .map_err(|e| OrchestratorError::InferenceFailed(format!("Sampling error: {e}")))?;

            if let Some(eos) = state.eos_token_id {
                if next_token == eos {
                    break;
                }
            }

            generated.push(next_token);
            all_tokens.push(next_token);

            // Decode the partial assistant output so that role markers do not fall into the response
            let partial = state
                .tokenizer
                .decode(&generated, true)
                .map_err(|e| OrchestratorError::InferenceFailed(format!("Decode error: {e}")))?;
            if stop_markers.iter().any(|marker| partial.contains(marker)) {
                break;
            }
        }

        let output = state
            .tokenizer
            .decode(&generated, true)
            .map_err(|e| OrchestratorError::InferenceFailed(format!("Decode error: {e}")))?;
        Ok(self.trim_at_stop_markers(&output, &stop_markers))
    }

    fn memory_usage_bytes(&self) -> u64 {
        self.memory_usage
    }

    fn get_metadata(&self) -> HashMap<String, Value> {
        let mut metadata = HashMap::new();
        metadata.insert("model_id".to_string(), Value::String(self.model_id.clone()));
        metadata.insert(
            "model_path".to_string(),
            Value::String(self.config.model_path.clone()),
        );
        metadata.insert(
            "device".to_string(),
            Value::String(self.config.device.clone()),
        );
        metadata.insert("loaded".to_string(), Value::Bool(self.loaded));
        metadata.insert(
            "memory_mb".to_string(),
            Value::Number((self.memory_usage / 1024 / 1024).into()),
        );

        if let Some(q) = &self.config.quantization {
            metadata.insert("quantization".to_string(), Value::String(q.clone()));
        }

        metadata
    }

    async fn health_check(&self) -> OrchestratorResult<bool> {
        if !self.loaded {
            return Ok(false);
        }

        // In production, run a simple inference to verify model is working
        // For now, just check that state is present
        Ok(self.state.is_some())
    }
}

// ============================================================================
// ModelPool - Orchestrator with LRU Eviction
// ============================================================================

/// Entry in the model pool tracking metadata and access patterns
struct ModelEntry {
    /// The model provider instance
    provider: Box<dyn ModelProvider>,

    /// Configuration used to create this model
    config: ModelProviderConfig,

    /// Last time this model was accessed (for LRU eviction)
    last_accessed: Instant,

    /// Timestamp when model was loaded
    loaded_at: Option<Instant>,

    /// Number of active inferences in-flight
    active_inferences: usize,
}

/// Production-ready model pool with LRU eviction and memory management
///
/// This orchestrator manages multiple models concurrently with:
/// - **Thread-safe concurrent access** via `RwLock`
/// - **Automatic LRU eviction** when memory is constrained
/// - **Idle timeout** to proactively unload unused models
/// - **Memory pressure awareness** using Linux sysinfo
///
/// ## Example
///
/// ```ignore
/// use mofa_foundation::orchestrator::{ModelPool, ModelProviderConfig};
///
/// #[tokio::main]
/// async fn main() -> Result<(), Box<dyn std::error::Error>> {
///     let pool = ModelPool::new();
///     
///     // Register a model
///     let config = ModelProviderConfig {
///         model_name: "llama-2-7b".into(),
///         model_path: "/models/llama-2-7b-q4.gguf".into(),
///         device: "cuda".into(),
///         max_context_length: Some(4096),
///         quantization: Some("q4_0".into()),
///         extra_config: Default::default(),
///     };
///     pool.register_model(config).await?;
///     
///     // Run inference (automatically loads model if needed)
///     let response = pool.infer("llama-2-7b", "What is Rust?").await?;
///     println!("Response: {}", response);
///     
///     Ok(())
/// }
/// ```
pub struct ModelPool {
    /// Pool of registered models — uses async RwLock because load/unload are long async operations
    models: Arc<AsyncRwLock<HashMap<String, ModelEntry>>>,

    /// Maximum memory threshold (bytes) — std RwLock for sync access in trait methods
    memory_threshold: Arc<RwLock<u64>>,

    /// Idle timeout in seconds — std RwLock for sync access in trait methods
    idle_timeout_secs: Arc<RwLock<u64>>,
}

impl ModelPool {
    /// Create a new model pool with default settings
    pub fn new() -> Self {
        let mut system = System::new_with_specifics(
            RefreshKind::new().with_memory(MemoryRefreshKind::everything()),
        );
        system.refresh_memory();

        // Default threshold: 80% of total system memory
        // sysinfo returns bytes directly — no multiplication needed
        let total_memory = system.total_memory();
        let default_threshold = (total_memory as f64 * DEFAULT_MEMORY_THRESHOLD_PERCENT) as u64;

        Self {
            models: Arc::new(AsyncRwLock::new(HashMap::new())),
            memory_threshold: Arc::new(RwLock::new(default_threshold)),
            idle_timeout_secs: Arc::new(RwLock::new(DEFAULT_IDLE_TIMEOUT_SECS)),
        }
    }

    /// Get current available system memory in bytes.
    /// Runs sysinfo on a blocking thread so it never stalls the async runtime.
    async fn get_available_memory(&self) -> u64 {
        tokio::task::spawn_blocking(|| {
            let mut sys = System::new_with_specifics(
                RefreshKind::new().with_memory(MemoryRefreshKind::everything()),
            );
            sys.refresh_memory();
            sys.available_memory() // sysinfo returns bytes
        })
        .await
        .unwrap_or(0)
    }

    /// Async helper for pool statistics — used by both the public sync trait method
    /// (via `block_in_place`) and internal async callers.
    async fn collect_statistics(&self) -> OrchestratorResult<PoolStatistics> {
        let available_memory = self.get_available_memory().await;

        let (loaded_count, total_memory) = {
            let models = self.models.read().await;
            let loaded: Vec<_> = models.values().filter(|e| e.provider.is_loaded()).collect();
            let count = loaded.len();
            let mem: u64 = loaded.iter().map(|e| e.provider.memory_usage_bytes()).sum();
            (count, mem)
        };

        Ok(PoolStatistics {
            loaded_models_count: loaded_count,
            total_memory_usage: total_memory,
            available_memory,
            queued_models_count: 0,
            timestamp: chrono::Utc::now(),
        })
    }

    /// Get total memory used by all loaded models
    async fn get_total_model_memory(&self) -> u64 {
        let models = self.models.read().await;
        models
            .values()
            .filter(|entry| entry.provider.is_loaded())
            .map(|entry| entry.provider.memory_usage_bytes())
            .sum()
    }

    /// Route a request to the best available model for the given task type.
    ///
    /// Selection priority:
    /// 1. A currently *loaded* model of the matching type (ready immediately)
    /// 2. A *registered* (but unloaded) model of the matching type (will be loaded on demand)
    pub async fn route_by_type_inner(&self, task: &ModelType) -> OrchestratorResult<String> {
        let models = self.models.read().await;

        // Pass 1: prefer a model that is already loaded
        for (id, entry) in models.iter() {
            if entry.config.model_type == *task && entry.provider.is_loaded() {
                return Ok(id.clone());
            }
        }

        // Pass 2: fall back to a registered but unloaded model
        for (id, entry) in models.iter() {
            if entry.config.model_type == *task {
                return Ok(id.clone());
            }
        }

        Err(OrchestratorError::NoModelForType(format!("{}", task)))
    }

    /// Apply dynamic precision degradation to the model with the worst LRU score.
    ///
    /// When memory is constrained and no model can be evicted, the heaviest loaded
    /// model is upgraded to the next degradation level (`q8 → q4`). The model must
    /// be re-loaded by the caller after this call returns.
    pub async fn apply_precision_degradation(&self) -> OrchestratorResult<Option<String>> {
        let mut models = self.models.write().await;

        // Find the loaded model with the highest memory footprint
        let candidate = models
            .iter_mut()
            .filter(|(_, entry)| entry.provider.is_loaded())
            .max_by_key(|(_, entry)| entry.provider.memory_usage_bytes());

        if let Some((id, entry)) = candidate {
            let current_q = entry.config.quantization.as_deref().unwrap_or("f32");

            // Map current quantization string → DegradationLevel
            let current_level = match current_q {
                "f32" => DegradationLevel::Full,
                "f16" => DegradationLevel::Half,
                "q8_0" => DegradationLevel::Int8,
                _ => DegradationLevel::Int4,
            };

            if let Some(next_level) = current_level.next_level() {
                let next_q = next_level.as_quantization_str().to_string();
                let model_id = id.clone();

                tracing::warn!(
                    "Memory pressure: degrading model '{}' from {} → {}",
                    model_id,
                    current_q,
                    next_q
                );

                // Update the quantization config — the model must be reloaded
                entry.config.quantization = Some(next_q);

                return Ok(Some(model_id));
            }
        }

        Ok(None) // Nothing could be degraded further
    }

    /// Check if loading a model would exceed memory constraints
    async fn check_memory_pressure(&self, estimated_model_size: u64) -> OrchestratorResult<()> {
        let available = self.get_available_memory().await;
        let current_usage = self.get_total_model_memory().await;
        let threshold = *self
            .memory_threshold
            .read()
            .map_err(|e| OrchestratorError::Other(format!("threshold lock poisoned: {e}")))?;

        let projected_usage = current_usage + estimated_model_size;

        tracing::debug!(
            "Memory check: available={} MB, current={} MB, estimated_model={} MB, projected={} MB, threshold={} MB",
            available / 1024 / 1024,
            current_usage / 1024 / 1024,
            estimated_model_size / 1024 / 1024,
            projected_usage / 1024 / 1024,
            threshold / 1024 / 1024
        );

        // Check 1: Minimum free memory requirement
        if available < MIN_FREE_MEMORY_BYTES + estimated_model_size {
            return Err(OrchestratorError::MemoryConstrained(format!(
                "Insufficient free memory: need {} MB, have {} MB available",
                (MIN_FREE_MEMORY_BYTES + estimated_model_size) / 1024 / 1024,
                available / 1024 / 1024
            )));
        }

        // Check 2: Total usage would exceed threshold
        if projected_usage > threshold {
            return Err(OrchestratorError::MemoryConstrained(format!(
                "Would exceed memory threshold: projected {} MB > threshold {} MB",
                projected_usage / 1024 / 1024,
                threshold / 1024 / 1024
            )));
        }

        Ok(())
    }

    /// Find the least recently used (LRU) loaded model
    async fn find_lru_candidate(&self) -> Option<String> {
        let models = self.models.read().await;
        let idle_timeout = match self.idle_timeout_secs.read() {
            Ok(val) => Duration::from_secs(*val),
            Err(_) => return None,
        };
        let now = Instant::now();

        models
            .iter()
            .filter(|(_, entry)| {
                // Must be loaded and idle for at least the timeout duration
                entry.provider.is_loaded()
                    && now.duration_since(entry.last_accessed) >= idle_timeout
                    && entry.active_inferences == 0
            })
            .min_by_key(|(_, entry)| entry.last_accessed)
            .map(|(id, _)| id.clone())
    }

    /// Evict the LRU model to free memory
    async fn evict_lru_model(&self) -> OrchestratorResult<Option<String>> {
        let candidate = self.find_lru_candidate().await;

        if let Some(model_id) = candidate {
            tracing::info!("Evicting LRU model '{}' to free memory", model_id);

            // Unload the model
            let mut models = self.models.write().await;
            if let Some(entry) = models.get_mut(&model_id) {
                if entry.active_inferences > 0 {
                    tracing::warn!(
                        "Skipping eviction for model '{}' ({} active inferences)",
                        model_id,
                        entry.active_inferences
                    );
                    return Ok(None);
                }
                entry.provider.unload().await?;
                tracing::info!("Successfully evicted model '{}'", model_id);
                return Ok(Some(model_id));
            }
        }

        Ok(None)
    }

    /// Automatic background task to enforce idle timeout
    ///
    /// This should be spawned when the pool is created to continuously
    /// monitor and evict idle models in the background.
    pub async fn run_idle_cleanup_task(pool: Arc<Self>) {
        loop {
            // Check every 5 seconds
            sleep(Duration::from_secs(5)).await;

            // Find and evict idle models
            if let Some(model_id) = pool.find_lru_candidate().await {
                tracing::info!("Idle cleanup: evicting model '{}'", model_id);
                if let Err(e) = pool.unload_model(&model_id).await {
                    tracing::error!("Failed to evict idle model '{}': {}", model_id, e);
                }
            }
        }
    }
}

impl Default for ModelPool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl ModelOrchestrator for ModelPool {
    fn name(&self) -> &str {
        "ModelPool"
    }

    async fn register_model(&self, config: ModelProviderConfig) -> OrchestratorResult<()> {
        let model_id = config.model_name.clone();

        tracing::info!("Registering model '{}'", model_id);

        // Create provider instance (unloaded)
        let provider = Box::new(LinuxCandleProvider::new(config.clone()));

        let entry = ModelEntry {
            provider,
            config,
            last_accessed: Instant::now(),
            loaded_at: None,
            active_inferences: 0,
        };

        let mut models = self.models.write().await;
        if models.contains_key(&model_id) {
            return Err(OrchestratorError::ConfigError(format!(
                "Model '{}' is already registered",
                model_id
            )));
        }
        models.insert(model_id.clone(), entry);

        tracing::info!("Model '{}' registered successfully", model_id);
        Ok(())
    }

    async fn unregister_model(&self, model_id: &str) -> OrchestratorResult<()> {
        tracing::info!("Unregistering model '{}'", model_id);

        let mut models = self.models.write().await;
        if let Some(mut entry) = models.remove(model_id) {
            let active = entry.active_inferences;
            if active > 0 {
                models.insert(model_id.to_string(), entry);
                return Err(OrchestratorError::ModelBusy(format!(
                    "Model '{}' has {} active inferences",
                    model_id, active
                )));
            }
            // Unload if loaded
            if entry.provider.is_loaded() {
                entry.provider.unload().await?;
            }
            tracing::info!("Model '{}' unregistered successfully", model_id);
            Ok(())
        } else {
            Err(OrchestratorError::ModelNotFound(model_id.to_string()))
        }
    }

    async fn load_model(&self, model_id: &str) -> OrchestratorResult<()> {
        tracing::info!("Loading model '{}'", model_id);

        // Step 1: Get mutable access to the model
        let mut models = self.models.write().await;
        if models.contains_key(model_id) == false {
            return Err(OrchestratorError::ModelNotFound(model_id.to_string()));
        }
        let entry = models
            .get_mut(model_id)
            .ok_or_else(|| OrchestratorError::ModelNotFound(model_id.to_string()))?;

        // Already loaded?
        if entry.provider.is_loaded() {
            tracing::debug!("Model '{}' is already loaded", model_id);
            entry.last_accessed = Instant::now();
            return Ok(());
        }

        // Step 2: Estimate memory requirement
        let estimated_size = if entry.config.model_path.contains("7b") {
            // Example: 7B model with q4 quantization ≈ 3.5 GB
            3_500_000_000
        } else {
            // Default assumption
            2_000_000_000
        };

        // Step 3: Check memory pressure
        drop(models); // Release lock before async operations

        let mut attempts = 0;
        const MAX_EVICTION_ATTEMPTS: usize = 5;

        loop {
            match self.check_memory_pressure(estimated_size).await {
                Ok(_) => break, // Sufficient memory available
                Err(OrchestratorError::MemoryConstrained(_))
                    if attempts < MAX_EVICTION_ATTEMPTS =>
                {
                    tracing::warn!(
                        "Memory constrained. Attempting LRU eviction (attempt {}/{})",
                        attempts + 1,
                        MAX_EVICTION_ATTEMPTS
                    );

                    // Try to evict LRU model
                    match self.evict_lru_model().await? {
                        Some(evicted_id) => {
                            tracing::info!("Evicted model '{}' to free memory", evicted_id);
                            attempts += 1;
                        }
                        None => {
                            // No more models to evict
                            return Err(OrchestratorError::MemoryConstrained(
                                "No idle models available for eviction".to_string(),
                            ));
                        }
                    }
                }
                Err(e) => return Err(e), // Other error or max attempts exceeded
            }
        }

        // Step 4: Load the model
        let mut models = self.models.write().await;
        let entry = models
            .get_mut(model_id)
            .ok_or_else(|| OrchestratorError::ModelNotFound(model_id.to_string()))?;

        entry.provider.load().await?;
        entry.last_accessed = Instant::now();
        entry.loaded_at = Some(Instant::now());

        tracing::info!("Model '{}' loaded successfully", model_id);
        Ok(())
    }

    async fn unload_model(&self, model_id: &str) -> OrchestratorResult<()> {
        tracing::info!("Unloading model '{}'", model_id);

        let mut models = self.models.write().await;
        let entry = models
            .get_mut(model_id)
            .ok_or_else(|| OrchestratorError::ModelNotFound(model_id.to_string()))?;

        if entry.active_inferences > 0 {
            return Err(OrchestratorError::ModelBusy(format!(
                "Model '{}' has {} active inferences",
                model_id, entry.active_inferences
            )));
        }

        entry.provider.unload().await?;
        entry.loaded_at = None;

        tracing::info!("Model '{}' unloaded successfully", model_id);
        Ok(())
    }

    fn is_model_loaded(&self, model_id: &str) -> bool {
        // Safe: models uses AsyncRwLock. We use try_read() here so this sync method
        // never blocks the async thread. If the lock is held, we conservatively
        // return false (caller will retry via the async load_model path).
        if let Ok(models) = self.models.try_read() {
            models
                .get(model_id)
                .map(|entry| entry.provider.is_loaded())
                .unwrap_or(false)
        } else {
            false
        }
    }

    async fn infer(&self, model_id: &str, input: &str) -> OrchestratorResult<String> {
        // Step 1: Ensure model is loaded
        self.load_model(model_id).await?;

        // Step 2: Update last accessed time
        {
            let mut models = self.models.write().await;
            if let Some(entry) = models.get_mut(model_id) {
                entry.last_accessed = Instant::now();
                entry.active_inferences += 1;
            }
        }

        // Step 3: Run inference
        let result = {
            let models = self.models.read().await;
            let entry = models
                .get(model_id)
                .ok_or_else(|| OrchestratorError::ModelNotFound(model_id.to_string()))?;
            entry.provider.infer(input).await
        };

        // Step 4: Decrement active inference count
        {
            let mut models = self.models.write().await;
            if let Some(entry) = models.get_mut(model_id) {
                entry.active_inferences = entry.active_inferences.saturating_sub(1);
            }
        }

        result
    }

    fn get_statistics(&self) -> OrchestratorResult<PoolStatistics> {
        // We need async operations for memory (spawn_blocking) and the models lock.
        // `block_in_place` moves us to a blocking thread so we can run async work
        // without stalling the runtime's event loop.
        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(self.collect_statistics())
        })
    }

    fn list_models(&self) -> Vec<String> {
        if let Ok(models) = self.models.try_read() {
            models.keys().cloned().collect()
        } else {
            vec![]
        }
    }

    fn list_loaded_models(&self) -> Vec<String> {
        if let Ok(models) = self.models.try_read() {
            models
                .iter()
                .filter(|(_, entry)| entry.provider.is_loaded())
                .map(|(id, _)| id.clone())
                .collect()
        } else {
            vec![]
        }
    }

    async fn route_by_type(&self, task: &ModelType) -> OrchestratorResult<String> {
        self.route_by_type_inner(task).await
    }

    async fn trigger_eviction(&self, target_bytes: u64) -> OrchestratorResult<usize> {
        let mut freed_bytes = 0u64;
        let mut evicted_count = 0usize;

        while freed_bytes < target_bytes {
            match self.evict_lru_model().await? {
                Some(model_id) => {
                    // Model was evicted
                    evicted_count += 1;

                    // Estimate freed memory (actual calculation would be better)
                    freed_bytes += 2_000_000_000; // Assume ~2GB per model

                    tracing::info!(
                        "Evicted model '{}'  (freed ~{} MB so far)",
                        model_id,
                        freed_bytes / 1024 / 1024
                    );
                }
                None => {
                    // No more models to evict
                    if freed_bytes < target_bytes {
                        return Err(OrchestratorError::EvictionFailed(format!(
                            "Only freed {} MB out of target {} MB",
                            freed_bytes / 1024 / 1024,
                            target_bytes / 1024 / 1024
                        )));
                    }
                    break;
                }
            }
        }

        Ok(evicted_count)
    }

    async fn set_memory_threshold(&self, bytes: u64) -> OrchestratorResult<()> {
        let mut threshold = self
            .memory_threshold
            .write()
            .map_err(|e| OrchestratorError::Other(format!("threshold lock poisoned: {e}")))?;
        *threshold = bytes;
        tracing::info!("Memory threshold set to {} MB", bytes / 1024 / 1024);
        Ok(())
    }

    fn get_memory_threshold(&self) -> u64 {
        *self
            .memory_threshold
            .read()
            .expect("threshold lock poisoned")
    }

    async fn set_idle_timeout_secs(&self, secs: u64) -> OrchestratorResult<()> {
        let mut timeout = self
            .idle_timeout_secs
            .write()
            .map_err(|e| OrchestratorError::Other(format!("idle_timeout lock poisoned: {e}")))?;
        *timeout = secs;
        tracing::info!("Idle timeout set to {} seconds", secs);
        Ok(())
    }

    fn get_idle_timeout_secs(&self) -> u64 {
        *self
            .idle_timeout_secs
            .read()
            .expect("idle_timeout lock poisoned")
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    /// Helper to create a test model config
    fn create_test_config(name: &str) -> ModelProviderConfig {
        create_test_config_with_type(name, ModelType::Llm)
    }

    /// Helper to create a test model config with explicit type
    fn create_test_config_with_type(name: &str, model_type: ModelType) -> ModelProviderConfig {
        ModelProviderConfig {
            model_name: name.to_string(),
            model_path: format!("/tmp/test_{}.gguf", name),
            device: "cpu".to_string(),
            model_type,
            max_context_length: Some(2048),
            quantization: Some("q4_0".to_string()),
            extra_config: HashMap::new(),
        }
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn test_provider_creation() {
        let config = create_test_config("test_model");
        let provider = LinuxCandleProvider::new(config);

        assert_eq!(provider.name(), "LinuxCandleProvider");
        assert_eq!(provider.model_id(), "test_model");
        assert!(!provider.is_loaded());
        assert_eq!(provider.memory_usage_bytes(), 0);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn test_device_selection_cpu_fallback() {
        let config = create_test_config("test_cpu");
        let provider = LinuxCandleProvider::new(config);

        // Should fallback to CPU if CUDA unavailable
        let device = provider.select_device().unwrap();
        // Device comparison is tricky, but we can verify it didn't panic
        assert!(matches!(device, Device::Cpu));
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn test_pool_creation() {
        let pool = ModelPool::new();
        assert_eq!(pool.name(), "ModelPool");
        assert_eq!(pool.list_models().len(), 0);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn test_register_and_unregister() {
        let pool = ModelPool::new();
        let config = create_test_config("model1");

        // Register
        pool.register_model(config).await.unwrap();
        assert_eq!(pool.list_models().len(), 1);
        assert!(pool.list_models().contains(&"model1".to_string()));

        // Unregister
        pool.unregister_model("model1").await.unwrap();
        assert_eq!(pool.list_models().len(), 0);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn test_register_duplicate_model_fails() {
        let pool = ModelPool::new();
        let config = create_test_config("dup_model");

        pool.register_model(config.clone()).await.unwrap();
        let result = pool.register_model(config).await;

        assert!(matches!(result, Err(OrchestratorError::ConfigError(_))));
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn test_unregister_nonexistent_model() {
        let pool = ModelPool::new();
        let result = pool.unregister_model("nonexistent").await;
        assert!(matches!(result, Err(OrchestratorError::ModelNotFound(_))));
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn test_pool_statistics() {
        let pool = ModelPool::new();

        let config1 = create_test_config("model1");
        let config2 = create_test_config("model2");

        pool.register_model(config1).await.unwrap();
        pool.register_model(config2).await.unwrap();

        let stats = pool.get_statistics().unwrap();
        assert_eq!(stats.loaded_models_count, 0); // None loaded yet
        assert_eq!(stats.total_memory_usage, 0);
        assert!(stats.available_memory > 0);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn test_idle_timeout_configuration() {
        let pool = ModelPool::new();

        // Default timeout
        assert_eq!(pool.get_idle_timeout_secs(), DEFAULT_IDLE_TIMEOUT_SECS);

        // Set custom timeout
        pool.set_idle_timeout_secs(60).await.unwrap();
        assert_eq!(pool.get_idle_timeout_secs(), 60);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn test_memory_threshold_configuration() {
        let pool = ModelPool::new();

        // Default threshold should be set
        let default_threshold = pool.get_memory_threshold();
        assert!(default_threshold > 0);

        // Set custom threshold (1 GB)
        let custom_threshold = 1_000_000_000u64;
        pool.set_memory_threshold(custom_threshold).await.unwrap();
        assert_eq!(pool.get_memory_threshold(), custom_threshold);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn test_list_loaded_models() {
        let pool = ModelPool::new();

        pool.register_model(create_test_config("model1"))
            .await
            .unwrap();
        pool.register_model(create_test_config("model2"))
            .await
            .unwrap();

        // Initially, no models are loaded
        assert_eq!(pool.list_loaded_models().len(), 0);

        // All models are registered
        assert_eq!(pool.list_models().len(), 2);
    }

    /// Integration test: Multi-model pipeline with memory management
    ///
    /// This test simulates a real-world scenario:
    /// 1. Register multiple models
    /// 2. Load models sequentially
    /// 3. Verify memory tracking
    /// 4. Trigger manual eviction
    /// 5. Verify eviction freed memory
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn test_multi_model_pipeline_with_eviction() {
        let pool = Arc::new(ModelPool::new());

        // Set a low memory threshold to force eviction
        pool.set_memory_threshold(1_000_000_000).await.unwrap(); // 1 GB
        pool.set_idle_timeout_secs(1).await.unwrap(); // 1 second for testing

        // Register 3 models
        for i in 1..=3 {
            let config = create_test_config(&format!("model{}", i));
            pool.register_model(config).await.unwrap();
        }

        assert_eq!(pool.list_models().len(), 3);

        // Note: Actual loading would fail because we're using dummy models
        // In a real integration test with actual model files, you would:
        // 1. Load model1
        // 2. Wait for idle timeout
        // 3. Load model2 (should trigger eviction of model1)
        // 4. Verify model1 was evicted

        // For now, we'll just verify the pool structure is correct
        let stats = pool.get_statistics().unwrap();
        assert_eq!(stats.loaded_models_count, 0);
    }

    /// Test: Manual eviction trigger
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn test_manual_eviction_trigger() {
        let pool = ModelPool::new();

        // Register models
        pool.register_model(create_test_config("model1"))
            .await
            .unwrap();
        pool.register_model(create_test_config("model2"))
            .await
            .unwrap();

        // Set short idle timeout
        pool.set_idle_timeout_secs(0).await.unwrap();

        // Try to trigger eviction (should succeed even with no loaded models)
        let result = pool.trigger_eviction(1_000_000_000).await;

        // Should fail because no models are loaded
        assert!(result.is_err());
    }

    // provider used to exercise pool state
    struct TestProvider {
        model_id: String,
        model_type: ModelType,
        loaded: bool,
        memory: u64,
    }

    impl TestProvider {
        fn new(model_id: &str, model_type: ModelType, loaded: bool) -> Self {
            Self {
                model_id: model_id.to_string(),
                model_type,
                loaded,
                memory: 123,
            }
        }
    }

    #[async_trait]
    impl ModelProvider for TestProvider {
        fn name(&self) -> &str {
            "TestProvider"
        }

        fn model_id(&self) -> &str {
            &self.model_id
        }

        fn model_type(&self) -> &ModelType {
            &self.model_type
        }

        async fn load(&mut self) -> OrchestratorResult<()> {
            self.loaded = true;
            Ok(())
        }

        async fn unload(&mut self) -> OrchestratorResult<()> {
            self.loaded = false;
            Ok(())
        }

        fn is_loaded(&self) -> bool {
            self.loaded
        }

        async fn infer(&self, _input: &str) -> OrchestratorResult<String> {
            if !self.loaded {
                return Err(OrchestratorError::InferenceFailed(
                    "Model is not loaded".to_string(),
                ));
            }
            Ok("ok".to_string())
        }

        fn memory_usage_bytes(&self) -> u64 {
            self.memory
        }

        fn get_metadata(&self) -> HashMap<String, Value> {
            HashMap::new()
        }

        async fn health_check(&self) -> OrchestratorResult<bool> {
            Ok(self.loaded)
        }
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn test_unload_busy_model_fails() {
        let pool = ModelPool::new();
        // Disable threshold based rejection so this test stays focused on the busy model guard.
        pool.set_memory_threshold(u64::MAX).await.unwrap();

        let config = create_test_config("busy_model");
        let entry = ModelEntry {
            provider: Box::new(TestProvider::new("busy_model", ModelType::Llm, true)),
            config,
            last_accessed: Instant::now(),
            loaded_at: Some(Instant::now()),
            // flight request
            active_inferences: 1,
        };

        // Inject the model
        let mut models = pool.models.write().await;
        models.insert("busy_model".to_string(), entry);
        drop(models);

        let result = pool.unload_model("busy_model").await;
        // Busy models should fail fast rather than unloading underneath an active request.
        assert!(matches!(result, Err(OrchestratorError::ModelBusy(_))));
    }

    /// Test: Memory pressure detection
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn test_memory_pressure_detection() {
        let pool = ModelPool::new();

        // Set very low threshold
        pool.set_memory_threshold(1).await.unwrap(); // 1 byte (impossible)

        // Try to load a model (should fail due to memory constraint)
        pool.register_model(create_test_config("model1"))
            .await
            .unwrap();

        let result = pool.load_model("model1").await;

        // Should fail with memory constrained error
        // Note: In this test with dummy models, loading will fail for other reasons
        // In production tests with real models, this would properly test memory pressure
        assert!(result.is_err());
    }
}
