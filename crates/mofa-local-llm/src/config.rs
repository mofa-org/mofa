//! Configuration for the Linux local inference backend

use crate::hardware::ComputeBackend;
use serde::{Deserialize, Serialize};

/// Configuration for the Linux local inference provider
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LinuxInferenceConfig {
    /// Path to the model weights file (GGUF or safetensors)
    pub model_path: String,

    /// Human-readable model name used as the provider ID
    pub model_name: String,

    /// Force a specific compute backend instead of auto-detecting.
    /// If None, the best available backend is selected automatically.
    pub backend_override: Option<ComputeBackend>,

    /// Maximum memory to allocate for the model in bytes.
    /// The provider will refuse to load if insufficient memory is available.
    /// If None, defaults to 80% of available system RAM.
    pub memory_limit_bytes: Option<u64>,

    /// Number of CPU threads to use for inference.
    /// Only relevant when the CPU backend is selected.
    /// If None, uses all available logical cores.
    pub num_threads: Option<usize>,

    /// Pin inference threads to specific CPU cores (comma-separated core IDs).
    /// Useful on multi-socket servers to avoid NUMA penalties.
    /// Example: `Some(vec![0, 1, 2, 3])`
    pub thread_affinity: Option<Vec<usize>>,

    /// Maximum number of tokens to generate per inference call
    pub max_tokens: usize,

    /// Sampling temperature (0.0 = greedy, 1.0 = creative)
    pub temperature: f32,

    /// Top-p nucleus sampling threshold
    pub top_p: f32,
}

impl Default for LinuxInferenceConfig {
    fn default() -> Self {
        Self {
            model_path: String::new(),
            model_name: String::from("local-model"),
            backend_override: None,
            memory_limit_bytes: None,
            num_threads: None,
            thread_affinity: None,
            max_tokens: 256,
            temperature: 0.8,
            top_p: 0.9,
        }
    }
}

impl LinuxInferenceConfig {
    pub fn new(model_name: impl Into<String>, model_path: impl Into<String>) -> Self {
        Self {
            model_name: model_name.into(),
            model_path: model_path.into(),
            ..Default::default()
        }
    }

    /// Force a specific compute backend
    pub fn with_backend(mut self, backend: ComputeBackend) -> Self {
        self.backend_override = Some(backend);
        self
    }

    /// Set the memory limit in bytes
    pub fn with_memory_limit(mut self, bytes: u64) -> Result<Self, &'static str> {
        if bytes == 0 {
            return Err("memory_limit_bytes must be > 0");
        }
        self.memory_limit_bytes = Some(bytes);
        Ok(self)
    }

    /// Set number of CPU threads
    pub fn with_num_threads(mut self, threads: usize) -> Result<Self, &'static str> {
        if threads == 0 {
            return Err("num_threads must be > 0");
        }
        self.num_threads = Some(threads);
        Ok(self)
    }

    /// Set CPU core affinity for inference threads
    pub fn with_thread_affinity(mut self, cores: Vec<usize>) -> Result<Self, &'static str> {
        if cores.is_empty() {
            return Err("thread_affinity must not be empty");
        }
        self.thread_affinity = Some(cores);
        Ok(self)
    }

    /// Set sampling temperature
    pub fn with_temperature(mut self, temp: f32) -> Result<Self, &'static str> {
        if !(0.0..=2.0).contains(&temp) {
            return Err("temperature must be between 0.0 and 2.0");
        }
        self.temperature = temp;
        Ok(self)
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let cfg = LinuxInferenceConfig::default();
        assert_eq!(cfg.max_tokens, 256);
        assert_eq!(cfg.temperature, 0.8);
        assert!(cfg.backend_override.is_none());
        assert!(cfg.memory_limit_bytes.is_none());
    }

    #[test]
    fn test_builder_backend_override() {
        let cfg = LinuxInferenceConfig::new("model", "/path/to/model")
            .with_backend(ComputeBackend::Cpu);
        assert_eq!(cfg.backend_override, Some(ComputeBackend::Cpu));
    }

    #[test]
    fn test_builder_memory_limit_valid() {
        let cfg = LinuxInferenceConfig::new("model", "/path")
            .with_memory_limit(4 * 1024 * 1024 * 1024)
            .unwrap();
        assert_eq!(cfg.memory_limit_bytes, Some(4 * 1024 * 1024 * 1024));
    }

    #[test]
    fn test_builder_memory_limit_zero_rejected() {
        let result = LinuxInferenceConfig::new("model", "/path").with_memory_limit(0);
        assert!(result.is_err());
    }

    #[test]
    fn test_builder_threads_zero_rejected() {
        let result = LinuxInferenceConfig::new("model", "/path").with_num_threads(0);
        assert!(result.is_err());
    }

    #[test]
    fn test_builder_affinity_empty_rejected() {
        let result = LinuxInferenceConfig::new("model", "/path").with_thread_affinity(vec![]);
        assert!(result.is_err());
    }

    #[test]
    fn test_builder_temperature_out_of_range() {
        let result = LinuxInferenceConfig::new("model", "/path").with_temperature(3.0);
        assert!(result.is_err());
    }

    #[test]
    fn test_builder_temperature_valid() {
        let cfg = LinuxInferenceConfig::new("model", "/path")
            .with_temperature(0.0)
            .unwrap();
        assert_eq!(cfg.temperature, 0.0);
    }

    #[test]
    fn test_config_serde_roundtrip() {
        let cfg = LinuxInferenceConfig::new("llama", "/models/llama.gguf")
            .with_backend(ComputeBackend::Rocm)
            .with_memory_limit(8 * 1024 * 1024 * 1024)
            .unwrap();
        let json = serde_json::to_string(&cfg).expect("serialize");
        let back: LinuxInferenceConfig = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(back.model_name, "llama");
        assert_eq!(back.backend_override, Some(ComputeBackend::Rocm));
    }
}
