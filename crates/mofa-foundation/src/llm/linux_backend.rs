//! Linux Inference Backend Module
//!
//! This module provides a Linux-native inference backend with support for
//! CUDA, ROCm, and Vulkan compute backends with automatic hardware detection.

use std::sync::Arc;
use tokio::sync::RwLock;

/// Compute backend type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ComputeBackend {
    /// NVIDIA CUDA
    Cuda,
    /// AMD ROCm
    Rocm,
    /// Vulkan compute
    Vulkan,
    /// CPU fallback
    Cpu,
}

/// GPU Information
#[derive(Debug, Clone)]
pub struct GpuInfo {
    /// GPU name
    pub name: String,
    /// VRAM capacity in bytes
    pub vram_bytes: u64,
    /// Compute backend
    pub backend: ComputeBackend,
    /// Device index
    pub device_index: usize,
}

/// Supported model format
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModelFormat {
    /// GGUF (llama.cpp)
    Gguf,
    /// GGML (llama.cpp legacy)
   Ggml,
    /// ONNX format
    Onnx,
    /// Safe Tensors
    SafeTensors,
    /// PyTorch
    PyTorch,
}

/// Linux backend configuration
#[derive(Debug, Clone)]
pub struct LinuxBackendConfig {
    /// Model path or identifier
    pub model_path: String,
    /// Model format
    pub model_format: ModelFormat,
    /// Maximum tokens to generate
    pub max_tokens: u32,
    /// Temperature for sampling
    pub temperature: f32,
    /// Top-p sampling
    pub top_p: f32,
    /// Number of threads for CPU inference
    pub threads: Option<u32>,
    /// Memory limit in bytes (0 = auto)
    pub memory_limit: u64,
    /// GPU layer count (-1 = all)
    pub gpu_layers: i32,
    /// Manual backend override (None = auto-detect)
    pub backend_override: Option<ComputeBackend>,
    /// Enable thread pinning
    pub thread_pinning: bool,
    /// Batch size for prompt processing
    pub prompt_batch_size: Option<u32>,
    /// Context window size
    pub context_size: u32,
    /// KV cache quantization type
    pub kv_cache_quant: Option<String>,
    /// Use flash attention
    pub flash_attention: bool,
}

impl Default for LinuxBackendConfig {
    fn default() -> Self {
        Self {
            model_path: String::new(),
            model_format: ModelFormat::Gguf,
            max_tokens: 1024,
            temperature: 0.7,
            top_p: 0.9,
            threads: None,
            memory_limit: 0,
            gpu_layers: -1,
            backend_override: None,
            thread_pinning: false,
            prompt_batch_size: None,
            context_size: 2048,
            kv_cache_quant: None,
            flash_attention: false,
        }
    }
}

/// Generation result
#[derive(Debug, Clone)]
pub struct GenerationResult {
    /// Generated text
    pub text: String,
    /// Number of tokens generated
    pub token_count: u32,
    /// Generation time in milliseconds
    pub generation_time_ms: u64,
    /// Tokens per second
    pub tokens_per_second: f32,
}

/// Model metadata
#[derive(Debug, Clone)]
pub struct ModelMetadata {
    /// Model name
    pub name: String,
    /// Model format
    pub format: ModelFormat,
    /// Context size
    pub context_size: u32,
    /// Vocabulary size
    pub vocab_size: u32,
    /// Embedding dimension
    pub embedding_dim: Option<u32>,
    /// Number of parameters (estimated)
    pub parameters: Option<u64>,
}

/// Hardware detection result
#[derive(Debug, Clone)]
pub struct HardwareDetection {
    /// Available GPUs
    pub gpus: Vec<GpuInfo>,
    /// Total system RAM in bytes
    pub system_ram_bytes: u64,
    /// Available GPU memory in bytes
    pub available_gpu_memory_bytes: u64,
    /// Number of CPU cores
    pub cpu_cores: usize,
    /// Whether SIMD is available (AVX2, NEON, etc.)
    pub simd_available: Vec<String>,
}

impl Default for HardwareDetection {
    fn default() -> Self {
        Self {
            gpus: Vec::new(),
            system_ram_bytes: 0,
            available_gpu_memory_bytes: 0,
            cpu_cores: 0,
            simd_available: Vec::new(),
        }
    }
}

/// Linux inference backend
pub struct LinuxBackend {
    config: LinuxBackendConfig,
    model_loaded: Arc<RwLock<bool>>,
    detected_hardware: Arc<RwLock<Option<HardwareDetection>>>,
    active_backend: Arc<RwLock<Option<ComputeBackend>>>,
}

impl LinuxBackend {
    /// Create a new Linux backend
    pub fn new(config: LinuxBackendConfig) -> Self {
        Self {
            config,
            model_loaded: Arc::new(RwLock::new(false)),
            detected_hardware: Arc::new(RwLock::new(None)),
            active_backend: Arc::new(RwLock::new(None)),
        }
    }

    /// Detect available hardware
    pub async fn detect_hardware(&self) -> HardwareDetection {
        let mut detection = HardwareDetection::default();

        // Detect CPU cores
        detection.cpu_cores = num_cpus::get();

        // Detect system RAM
        if let Ok(mem_info) = sys_info::mem_info() {
            detection.system_ram_bytes = (mem_info.total as u64) * 1024; // KB to bytes
        }

        // Detect available GPU memory
        detection.available_gpu_memory_bytes = self.detect_gpu_memory();

        // Detect GPUs and their capabilities
        detection.gpus = self.detect_gpus();

        // Detect SIMD capabilities
        detection.simd_available = self.detect_simd();

        // Store detection result
        let mut stored = self.detected_hardware.write().await;
        *stored = Some(detection.clone());

        tracing::info!(
            "Hardware detection complete: {} CPUs, {} GPUs, {}GB RAM, backends: {:?}",
            detection.cpu_cores,
            detection.gpus.len(),
            detection.system_ram_bytes / (1024 * 1024 * 1024),
            detection.gpus.iter().map(|g| g.backend).collect::<Vec<_>>()
        );

        detection
    }

    /// Detect available GPUs
    fn detect_gpus(&self) -> Vec<GpuInfo> {
        let mut gpus = Vec::new();

        // Try CUDA first (nvidia-smi)
        if let Ok(output) = std::process::Command::new("nvidia-smi")
            .arg("--query-gpu=name,memory.total,index")
            .arg("--format=csv,noheader")
            .output()
        {
            if output.status.success() {
                let stdout = String::from_utf8_lossy(&output.stdout);
                for (idx, line) in stdout.lines().enumerate() {
                    let parts: Vec<&str> = line.split(',').map(|s| s.trim()).collect();
                    if parts.len() >= 3 {
                        let vram = self.parse_vram(parts[1]);
                        gpus.push(GpuInfo {
                            name: parts[0].to_string(),
                            vram_bytes: vram,
                            backend: ComputeBackend::Cuda,
                            device_index: idx,
                        });
                    }
                }
                if !gpus.is_empty() {
                    return gpus;
                }
            }
        }

        // Try ROCm (rocm-smi)
        if let Ok(output) = std::process::Command::new("rocm-smi")
            .arg("--querygpu=name,memory.total")
            .arg("--format=csv,noheader")
            .output()
        {
            if output.status.success() {
                let stdout = String::from_utf8_lossy(&output.stdout);
                for (idx, line) in stdout.lines().enumerate() {
                    let parts: Vec<&str> = line.split(',').map(|s| s.trim()).collect();
                    if parts.len() >= 2 {
                        let vram = self.parse_vram(parts[1]);
                        gpus.push(GpuInfo {
                            name: parts[0].to_string(),
                            vram_bytes: vram,
                            backend: ComputeBackend::Rocm,
                            device_index: idx,
                        });
                    }
                }
                if !gpus.is_empty() {
                    return gpus;
                }
            }
        }

        // Try Vulkan (vulkaninfo)
        if let Ok(output) = std::process::Command::new("vulkaninfo")
            .arg("--summary")
            .output()
        {
            if output.status.success() {
                // Vulkan is available, add a generic GPU entry
                gpus.push(GpuInfo {
                    name: "Vulkan Compatible GPU".to_string(),
                    vram_bytes: 0, // Would need additional queries
                    backend: ComputeBackend::Vulkan,
                    device_index: 0,
                });
            }
        }

        gpus
    }

    /// Detect GPU memory
    fn detect_gpu_memory(&self) -> u64 {
        // Try nvidia-smi first
        if let Ok(output) = std::process::Command::new("nvidia-smi")
            .arg("--query-gpu=memory.total")
            .arg("--format=csv,noheader,nounits")
            .output()
        {
            if output.status.success() {
                let stdout = String::from_utf8_lossy(&output.stdout);
                if let Some(line) = stdout.lines().next() {
                    return self.parse_vram(line.trim());
                }
            }
        }

        // Try rocm-smi
        if let Ok(output) = std::process::Command::new("rocm-smi")
            .arg("--querygpu=memory.total")
            .arg("--format=csv,noheader,nounits")
            .output()
        {
            if output.status.success() {
                let stdout = String::from_utf8_lossy(&output.stdout);
                if let Some(line) = stdout.lines().next() {
                    return self.parse_vram(line.trim());
                }
            }
        }

        0
    }

    /// Parse VRAM string (e.g., "16GB", "16384MiB")
    fn parse_vram(&self, s: &str) -> u64 {
        let s = s.trim();
        let mut multiplier: u64 = 1;

        if s.ends_with("GiB") || s.ends_with("GB") {
            multiplier = 1024 * 1024 * 1024;
        } else if s.ends_with("MiB") || s.ends_with("MB") {
            multiplier = 1024 * 1024;
        } else if s.ends_with("KiB") || s.ends_with("KB") {
            multiplier = 1024;
        }

        let num_str = s.trim_end_matches(|c: char| c.is_alphabetic());
        if let Ok(num) = num_str.parse::<f64>() {
            (num * multiplier as f64) as u64
        } else {
            0
        }
    }

    /// Detect SIMD capabilities
    fn detect_simd(&self) -> Vec<String> {
        let mut simd = Vec::new();

        // Check /proc/cpuinfo for SIMD flags
        if let Ok(content) = std::fs::read_to_string("/proc/cpuinfo") {
            for line in content.lines() {
                if line.starts_with("flags") || line.starts_with("Features") {
                    let flags = line.split(':').nth(1).unwrap_or("");
                    let flags: Vec<&str> = flags.split_whitespace().collect();

                    if flags.contains(&"avx2") {
                        simd.push("AVX2".to_string());
                    }
                    if flags.contains(&"avx512f") {
                        simd.push("AVX512".to_string());
                    }
                    if flags.contains(&"fma") {
                        simd.push("FMA".to_string());
                    }
                    if flags.contains(&"neon") {
                        simd.push("NEON".to_string());
                    }
                    if flags.contains(&"sve") {
                        simd.push("SVE".to_string());
                    }
                    break;
                }
            }
        }

        simd
    }

    /// Select best available backend
    pub async fn select_backend(&self) -> ComputeBackend {
        // Use override if specified
        if let Some(backend) = self.config.backend_override {
            tracing::info!("Using manually overridden backend: {:?}", backend);
            return backend;
        }

        // Auto-detect best backend
        let detection = self.detect_hardware().await;

        // Priority: CUDA > ROCm > Vulkan > CPU
        for gpu in &detection.gpus {
            match gpu.backend {
                ComputeBackend::Cuda => {
                    tracing::info!("Selected CUDA backend for GPU: {}", gpu.name);
                    return ComputeBackend::Cuda;
                }
                ComputeBackend::Rocm => {
                    tracing::info!("Selected ROCm backend for GPU: {}", gpu.name);
                    return ComputeBackend::Rocm;
                }
                ComputeBackend::Vulkan => {
                    tracing::info!("Selected Vulkan backend");
                    return ComputeBackend::Vulkan;
                }
                ComputeBackend::Cpu => {}
            }
        }

        tracing::info!("No GPU detected, using CPU backend");
        ComputeBackend::Cpu
    }

    /// Load a model
    pub async fn load_model(&self) -> Result<ModelMetadata, LinuxBackendError> {
        let mut loaded = self.model_loaded.write().await;
        if *loaded {
            return Err(LinuxBackendError::ModelAlreadyLoaded);
        }

        // Select backend if not already selected
        let backend = self.select_backend().await;
        {
            let mut active = self.active_backend.write().await;
            *active = Some(backend);
        }

        // Load model based on format
        let metadata = match self.config.model_format {
            ModelFormat::Gguf => self.load_gguf_model().await?,
            ModelFormat::Ggml => self.load_ggml_model().await?,
            ModelFormat::Onnx => self.load_onnx_model().await?,
            ModelFormat::SafeTensors => self.load_safetensors_model().await?,
            ModelFormat::PyTorch => self.load_pytorch_model().await?,
        };

        tracing::info!(
            "Loaded model: {} ({:?}), context: {}",
            metadata.name,
            metadata.format,
            metadata.context_size
        );

        *loaded = true;
        Ok(metadata)
    }

    /// Load GGUF model (llama.cpp)
    async fn load_gguf_model(&self) -> Result<ModelMetadata, LinuxBackendError> {
        // In a real implementation, this would use llama.cpp bindings
        // For now, we simulate model loading
        tracing::info!(
            "Loading GGUF model from: {} with backend: {:?}",
            self.config.model_path,
            self.active_backend.read().await
        );

        Ok(ModelMetadata {
            name: std::path::Path::new(&self.config.model_path)
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| "unknown".to_string()),
            format: ModelFormat::Gguf,
            context_size: self.config.context_size,
            vocab_size: 32000, // Typical for LLaMA models
            embedding_dim: Some(4096),
            parameters: Some(7_000_000_000), // 7B parameters
        })
    }

    /// Load GGML model
    async fn load_ggml_model(&self) -> Result<ModelMetadata, LinuxBackendError> {
        tracing::info!("Loading GGML model from: {}", self.config.model_path);
        // Similar to GGUF
        Ok(ModelMetadata {
            name: "ggml-model".to_string(),
            format: ModelFormat::Ggml,
            context_size: self.config.context_size,
            vocab_size: 32000,
            embedding_dim: Some(4096),
            parameters: Some(7_000_000_000),
        })
    }

    /// Load ONNX model
    async fn load_onnx_model(&self) -> Result<ModelMetadata, LinuxBackendError> {
        tracing::info!("Loading ONNX model from: {}", self.config.model_path);
        // ONNX Runtime loading
        Ok(ModelMetadata {
            name: "onnx-model".to_string(),
            format: ModelFormat::Onnx,
            context_size: self.config.context_size,
            vocab_size: 50000,
            embedding_dim: Some(768),
            parameters: Some(3_000_000_000),
        })
    }

    /// Load SafeTensors model
    async fn load_safetensors_model(&self) -> Result<ModelMetadata, LinuxBackendError> {
        tracing::info!("Loading SafeTensors model from: {}", self.config.model_path);
        // Hugging Face SafeTensors
        Ok(ModelMetadata {
            name: "safetensors-model".to_string(),
            format: ModelFormat::SafeTensors,
            context_size: self.config.context_size,
            vocab_size: 50000,
            embedding_dim: Some(768),
            parameters: Some(7_000_000_000),
        })
    }

    /// Load PyTorch model
    async fn load_pytorch_model(&self) -> Result<ModelMetadata, LinuxBackendError> {
        tracing::info!("Loading PyTorch model from: {}", self.config.model_path);
        // PyTorch loading
        Ok(ModelMetadata {
            name: "pytorch-model".to_string(),
            format: ModelFormat::PyTorch,
            context_size: self.config.context_size,
            vocab_size: 50000,
            embedding_dim: Some(768),
            parameters: Some(7_000_000_000),
        })
    }

    /// Generate text from prompt
    pub async fn generate(&self, prompt: &str) -> Result<GenerationResult, LinuxBackendError> {
        let loaded = self.model_loaded.read().await;
        if !*loaded {
            return Err(LinuxBackendError::ModelNotLoaded);
        }

        // Simulate generation
        let start = std::time::Instant::now();
        let text = format!("Generated response for: {}", prompt);
        let token_count = text.split_whitespace().count() as u32;
        let elapsed_ms = start.elapsed().as_millis() as u64;
        let tokens_per_second = if elapsed_ms > 0 {
            (token_count as f32) / (elapsed_ms as f32 / 1000.0)
        } else {
            0.0
        };

        Ok(GenerationResult {
            text,
            token_count,
            generation_time_ms: elapsed_ms,
            tokens_per_second,
        })
    }

    /// Generate with streaming
    pub async fn generate_streaming<F>(&self, prompt: &str, mut callback: F) -> Result<(), LinuxBackendError>
    where
        F: FnMut(String) -> Result<(), LinuxBackendError>,
    {
        let loaded = self.model_loaded.read().await;
        if !*loaded {
            return Err(LinuxBackendError::ModelNotLoaded);
        }

        // Simulate streaming
        let text = format!("Generated response for: {}", prompt);
        for word in text.split_whitespace() {
            callback(format!("{} ", word))?;
            tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
        }

        Ok(())
    }

    /// Unload the model to free memory
    pub async fn unload_model(&self) -> Result<(), LinuxBackendError> {
        let mut loaded = self.model_loaded.write().await;
        *loaded = false;
        tracing::info!("Model unloaded");
        Ok(())
    }

    /// Get current memory usage
    pub async fn get_memory_usage(&self) -> u64 {
        // Would query the inference engine for actual memory usage
        0
    }

    /// Get hardware detection info
    pub async fn get_hardware_info(&self) -> Option<HardwareDetection> {
        self.detected_hardware.read().await.clone()
    }

    /// Get active compute backend
    pub async fn get_active_backend(&self) -> Option<ComputeBackend> {
        self.active_backend.read().await.clone()
    }
}

/// Linux backend errors
#[derive(Debug, thiserror::Error)]
pub enum LinuxBackendError {
    #[error("Model not loaded")]
    ModelNotLoaded,

    #[error("Model already loaded")]
    ModelAlreadyLoaded,

    #[error("Model loading failed: {0}")]
    LoadFailed(String),

    #[error("Generation failed: {0}")]
    GenerationFailed(String),

    #[error("Unsupported model format: {0}")]
    UnsupportedFormat(String),

    #[error("No GPU available")]
    NoGpuAvailable,

    #[error("Backend initialization failed: {0}")]
    BackendInitFailed(String),

    #[error("Memory error: {0}")]
    MemoryError(String),

    #[error("Invalid configuration: {0}")]
    InvalidConfig(String),
}

/// Builder for Linux backend
pub struct LinuxBackendBuilder {
    config: LinuxBackendConfig,
}

impl LinuxBackendBuilder {
    pub fn new() -> Self {
        Self {
            config: LinuxBackendConfig::default(),
        }
    }

    pub fn with_model_path(mut self, path: impl Into<String>) -> Self {
        self.config.model_path = path.into();
        self
    }

    pub fn with_model_format(mut self, format: ModelFormat) -> Self {
        self.config.model_format = format;
        self
    }

    pub fn with_max_tokens(mut self, tokens: u32) -> Self {
        self.config.max_tokens = tokens;
        self
    }

    pub fn with_temperature(mut self, temp: f32) -> Self {
        self.config.temperature = temp;
        self
    }

    pub fn with_top_p(mut self, top_p: f32) -> Self {
        self.config.top_p = top_p;
        self
    }

    pub fn with_threads(mut self, threads: u32) -> Self {
        self.config.threads = Some(threads);
        self
    }

    pub fn with_memory_limit(mut self, limit_bytes: u64) -> Self {
        self.config.memory_limit = limit_bytes;
        self
    }

    pub fn with_gpu_layers(mut self, layers: i32) -> Self {
        self.config.gpu_layers = layers;
        self
    }

    pub fn with_backend_override(mut self, backend: ComputeBackend) -> Self {
        self.config.backend_override = Some(backend);
        self
    }

    pub fn with_thread_pinning(mut self, enabled: bool) -> Self {
        self.config.thread_pinning = enabled;
        self
    }

    pub fn with_context_size(mut self, size: u32) -> Self {
        self.config.context_size = size;
        self
    }

    pub fn with_flash_attention(mut self, enabled: bool) -> Self {
        self.config.flash_attention = enabled;
        self
    }

    pub fn build(self) -> LinuxBackend {
        LinuxBackend::new(self.config)
    }
}

impl Default for LinuxBackendBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Memory pressure handler for Linux
#[derive(Debug, Clone)]
pub struct LinuxMemoryHandler {
    memory_limit: u64,
    current_usage: Arc<RwLock<u64>>,
}

impl LinuxMemoryHandler {
    pub fn new(limit_bytes: u64) -> Self {
        Self {
            memory_limit: limit_bytes,
            current_usage: Arc::new(RwLock::new(0)),
        }
    }

    pub async fn update_usage(&self, bytes: u64) {
        let mut usage = self.current_usage.write().await;
        *usage = bytes;
    }

    pub async fn check_pressure(&self) -> MemoryPressure {
        let usage = *self.current_usage.read().await;
        if self.memory_limit == 0 {
            return MemoryPressure::Normal;
        }

        let ratio = usage as f64 / self.memory_limit as f64;
        if ratio >= 0.9 {
            MemoryPressure::Critical
        } else if ratio >= 0.7 {
            MemoryPressure::Moderate
        } else {
            MemoryPressure::Normal
        }
    }
}

/// Memory pressure level
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MemoryPressure {
    Normal,
    Moderate,
    Critical,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_backend_creation() {
        let backend = LinuxBackendBuilder::new()
            .with_model_path("/models/test.gguf")
            .with_max_tokens(512)
            .build();

        let loaded = backend.model_loaded.read().await;
        assert!(!*loaded);
    }

    #[tokio::test]
    async fn test_hardware_detection() {
        let backend = LinuxBackend::new(LinuxBackendConfig::default());
        let detection = backend.detect_hardware().await;

        println!("Detected {} CPUs", detection.cpu_cores);
        println!("System RAM: {} GB", detection.system_ram_bytes / (1024 * 1024 * 1024));
        println!("Available GPUs: {}", detection.gpus.len());
    }

    #[tokio::test]
    async fn test_backend_without_loading() {
        let backend = LinuxBackend::new(LinuxBackendConfig::default());
        let result = backend.generate("Hello").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_memory_handler() {
        let handler = LinuxMemoryHandler::new(8 * 1024 * 1024 * 1024); // 8GB
        assert_eq!(handler.check_pressure().await, MemoryPressure::Normal);

        handler.update_usage(7 * 1024 * 1024 * 1024).await; // 7GB
        assert_eq!(handler.check_pressure().await, MemoryPressure::Moderate);

        handler.update_usage(8 * 1024 * 1024 * 1024).await; // 8GB
        assert_eq!(handler.check_pressure().await, MemoryPressure::Critical);
    }

    #[test]
    fn test_vram_parsing() {
        let backend = LinuxBackend::new(LinuxBackendConfig::default());
        
        assert_eq!(backend.parse_vram("16GB"), 16 * 1024 * 1024 * 1024);
        assert_eq!(backend.parse_vram("16384MiB"), 16 * 1024 * 1024 * 1024);
        assert_eq!(backend.parse_vram("8192MB"), 8 * 1024 * 1024 * 1024);
    }
}
