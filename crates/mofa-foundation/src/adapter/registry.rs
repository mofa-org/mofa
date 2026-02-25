//! Adapter Registry — runtime discovery and deterministic resolution.
//!
//! The registry stores [`AdapterDescriptor`]s and resolves the best candidate
//! for a given [`ModelConfig`] using strict hard-constraint filtering followed
//! by deterministic tie-breaking (alphabetical by adapter ID).
//!
//! # Resolution Algorithm
//!
//! 1. **Modality filter** — adapter must support the requested modality.
//! 2. **Format filter** — adapter must support the requested model format.
//! 3. **Quantization filter** — if the request specifies a quantization profile,
//!    the adapter must list it as supported (adapters with an empty quantization
//!    set are treated as "accepts any").
//! 4. **Tie-break** — among all surviving candidates, the adapter with the
//!    lexicographically smallest `id` wins. This guarantees deterministic,
//!    reproducible resolution regardless of insertion order.

use std::collections::BTreeMap;

use super::descriptor::{AdapterDescriptor, ModelFormat, ModelModality};

/// Configuration describing the model a caller wants to run.
#[derive(Debug, Clone)]
pub struct ModelConfig {
    /// The modality required (e.g., `TextGeneration`)
    pub modality: ModelModality,
    /// The file format available on disk (e.g., `Gguf`)
    pub format: ModelFormat,
    /// Optional quantization profile (e.g., `"q4_0"`)
    pub quantization: Option<String>,
}

impl ModelConfig {
    /// Create a new model configuration.
    pub fn new(modality: ModelModality, format: ModelFormat) -> Self {
        Self {
            modality,
            format,
            quantization: None,
        }
    }

    /// Builder: set quantization requirement.
    pub fn with_quantization(mut self, quant: impl Into<String>) -> Self {
        self.quantization = Some(quant.into());
        self
    }
}

/// Errors produced during adapter resolution.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResolutionError {
    /// No adapters are registered at all.
    EmptyRegistry,
    /// No adapter supports the requested modality.
    ModalityNotSupported(ModelModality),
    /// No adapter supports the requested model format.
    FormatNotSupported(ModelFormat),
    /// No adapter supports the requested quantization profile.
    QuantizationNotSupported(String),
    /// No adapter passed all hard constraints.
    NoCompatibleAdapter {
        modality: ModelModality,
        format: ModelFormat,
        quantization: Option<String>,
    },
}

impl std::fmt::Display for ResolutionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EmptyRegistry => write!(f, "adapter registry is empty"),
            Self::ModalityNotSupported(m) => {
                write!(f, "no adapter supports modality: {}", m)
            }
            Self::FormatNotSupported(fmt) => {
                write!(f, "no adapter supports format: {}", fmt)
            }
            Self::QuantizationNotSupported(q) => {
                write!(f, "no adapter supports quantization: {}", q)
            }
            Self::NoCompatibleAdapter {
                modality,
                format,
                quantization,
            } => {
                write!(
                    f,
                    "no adapter matches all constraints: modality={}, format={}{}",
                    modality,
                    format,
                    quantization
                        .as_ref()
                        .map(|q| format!(", quant={}", q))
                        .unwrap_or_default()
                )
            }
        }
    }
}

/// The runtime model-adapter registry.
///
/// Adapters are registered with an [`AdapterDescriptor`] and can be looked up
/// deterministically using [`resolve`](AdapterRegistry::resolve).
///
/// # Example
///
/// ```
/// use mofa_foundation::adapter::{
///     AdapterDescriptor, AdapterRegistry, ModelConfig, ModelModality, ModelFormat,
/// };
///
/// let mut registry = AdapterRegistry::new();
///
/// // Register an MLX adapter that handles GGUF text-generation models
/// let mlx = AdapterDescriptor::new("mlx-local", "MLX Backend")
///     .with_modality(ModelModality::TextGeneration)
///     .with_format(ModelFormat::Gguf)
///     .with_quantization("q4_0");
///
/// registry.register(mlx);
///
/// let config = ModelConfig::new(ModelModality::TextGeneration, ModelFormat::Gguf);
/// let result = registry.resolve(&config);
/// assert!(result.is_ok());
/// assert_eq!(result.unwrap().id, "mlx-local");
/// ```
pub struct AdapterRegistry {
    /// BTreeMap ensures iteration is always sorted by adapter ID,
    /// giving us deterministic resolution for free.
    adapters: BTreeMap<String, AdapterDescriptor>,
}

impl AdapterRegistry {
    /// Create an empty registry.
    pub fn new() -> Self {
        Self {
            adapters: BTreeMap::new(),
        }
    }

    /// Register an adapter. If an adapter with the same ID already exists,
    /// it will be replaced.
    pub fn register(&mut self, descriptor: AdapterDescriptor) {
        self.adapters.insert(descriptor.id.clone(), descriptor);
    }

    /// Remove an adapter by ID. Returns `true` if it was present.
    pub fn unregister(&mut self, id: &str) -> bool {
        self.adapters.remove(id).is_some()
    }

    /// Get a reference to a registered adapter by ID.
    pub fn get(&self, id: &str) -> Option<&AdapterDescriptor> {
        self.adapters.get(id)
    }

    /// Number of registered adapters.
    pub fn len(&self) -> usize {
        self.adapters.len()
    }

    /// Whether the registry has no adapters.
    pub fn is_empty(&self) -> bool {
        self.adapters.is_empty()
    }

    /// List all registered adapter IDs.
    pub fn adapter_ids(&self) -> Vec<&str> {
        self.adapters.keys().map(|s| s.as_str()).collect()
    }

    /// Resolve the best adapter for the given model configuration.
    ///
    /// Uses strict hard-constraint filtering:
    /// 1. Modality must match.
    /// 2. Format must match.
    /// 3. Quantization must match (if specified in config).
    ///
    /// Among all passing candidates, the one with the lexicographically
    /// smallest `id` is returned (deterministic tie-break).
    pub fn resolve(&self, config: &ModelConfig) -> Result<&AdapterDescriptor, ResolutionError> {
        if self.adapters.is_empty() {
            return Err(ResolutionError::EmptyRegistry);
        }

        // BTreeMap iterates in sorted key order, so the first match is
        // already the deterministic winner.
        for descriptor in self.adapters.values() {
            if !descriptor.modalities.contains(&config.modality) {
                continue;
            }
            if !descriptor.supported_formats.contains(&config.format) {
                continue;
            }
            if let Some(ref quant) = config.quantization {
                // If the adapter has an explicit quantization set,
                // the requested quant must be in it.
                // An empty set means "accepts any quantization".
                if !descriptor.supported_quantization.is_empty()
                    && !descriptor.supported_quantization.contains(quant)
                {
                    continue;
                }
            }
            return Ok(descriptor);
        }

        // No candidate survived all filters — produce a specific error.
        let has_modality = self
            .adapters
            .values()
            .any(|d| d.modalities.contains(&config.modality));
        if !has_modality {
            return Err(ResolutionError::ModalityNotSupported(config.modality));
        }

        let has_format = self
            .adapters
            .values()
            .any(|d| d.supported_formats.contains(&config.format));
        if !has_format {
            return Err(ResolutionError::FormatNotSupported(config.format));
        }

        if let Some(ref quant) = config.quantization {
            let has_quant = self.adapters.values().any(|d| {
                d.supported_quantization.is_empty() || d.supported_quantization.contains(quant)
            });
            if !has_quant {
                return Err(ResolutionError::QuantizationNotSupported(quant.clone()));
            }
        }

        Err(ResolutionError::NoCompatibleAdapter {
            modality: config.modality,
            format: config.format,
            quantization: config.quantization.clone(),
        })
    }
}

impl Default for AdapterRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapter::descriptor::{ModelFormat, ModelModality};

    fn mlx_adapter() -> AdapterDescriptor {
        AdapterDescriptor::new("mlx-local", "MLX Backend")
            .with_modality(ModelModality::TextGeneration)
            .with_format(ModelFormat::Gguf)
            .with_format(ModelFormat::Safetensors)
            .with_quantization("q4_0")
            .with_quantization("q8_0")
    }

    fn llama_cpp_adapter() -> AdapterDescriptor {
        AdapterDescriptor::new("llama-cpp", "llama.cpp Backend")
            .with_modality(ModelModality::TextGeneration)
            .with_format(ModelFormat::Gguf)
            .with_quantization("q4_0")
            .with_quantization("q4_1")
            .with_quantization("q8_0")
    }

    fn whisper_adapter() -> AdapterDescriptor {
        AdapterDescriptor::new("whisper-mlx", "Whisper MLX")
            .with_modality(ModelModality::SpeechToText)
            .with_format(ModelFormat::Safetensors)
    }

    fn cloud_adapter() -> AdapterDescriptor {
        AdapterDescriptor::new("openai-api", "OpenAI API")
            .with_modality(ModelModality::TextGeneration)
            .with_modality(ModelModality::Embedding)
            .with_format(ModelFormat::Safetensors)
            .with_format(ModelFormat::Gguf)
    }

    // --- Happy path tests ---

    #[test]
    fn test_resolve_single_adapter() {
        let mut registry = AdapterRegistry::new();
        registry.register(mlx_adapter());

        let config = ModelConfig::new(ModelModality::TextGeneration, ModelFormat::Gguf);
        let result = registry.resolve(&config);
        assert!(result.is_ok());
        assert_eq!(result.unwrap().id, "mlx-local");
    }

    #[test]
    fn test_resolve_with_quantization() {
        let mut registry = AdapterRegistry::new();
        registry.register(mlx_adapter());

        let config = ModelConfig::new(ModelModality::TextGeneration, ModelFormat::Gguf)
            .with_quantization("q4_0");
        let result = registry.resolve(&config);
        assert!(result.is_ok());
        assert_eq!(result.unwrap().id, "mlx-local");
    }

    #[test]
    fn test_deterministic_tiebreak_alphabetical() {
        let mut registry = AdapterRegistry::new();
        // Register in non-alphabetical order
        registry.register(mlx_adapter());
        registry.register(llama_cpp_adapter());

        // Both support TextGeneration + Gguf + q4_0
        let config = ModelConfig::new(ModelModality::TextGeneration, ModelFormat::Gguf)
            .with_quantization("q4_0");
        let result = registry.resolve(&config);
        assert!(result.is_ok());
        // "llama-cpp" < "mlx-local" alphabetically
        assert_eq!(result.unwrap().id, "llama-cpp");
    }

    #[test]
    fn test_resolve_speech_to_text() {
        let mut registry = AdapterRegistry::new();
        registry.register(mlx_adapter());
        registry.register(whisper_adapter());

        let config = ModelConfig::new(ModelModality::SpeechToText, ModelFormat::Safetensors);
        let result = registry.resolve(&config);
        assert!(result.is_ok());
        assert_eq!(result.unwrap().id, "whisper-mlx");
    }

    #[test]
    fn test_cloud_adapter_accepts_any_quantization() {
        let mut registry = AdapterRegistry::new();
        registry.register(cloud_adapter());

        // Cloud adapter has empty quantization set → accepts anything
        let config = ModelConfig::new(ModelModality::TextGeneration, ModelFormat::Gguf)
            .with_quantization("q4_0");
        let result = registry.resolve(&config);
        assert!(result.is_ok());
        assert_eq!(result.unwrap().id, "openai-api");
    }

    // --- Error path tests ---

    #[test]
    fn test_empty_registry_error() {
        let registry = AdapterRegistry::new();
        let config = ModelConfig::new(ModelModality::TextGeneration, ModelFormat::Gguf);
        let err = registry.resolve(&config).unwrap_err();
        assert_eq!(err, ResolutionError::EmptyRegistry);
    }

    #[test]
    fn test_modality_not_supported() {
        let mut registry = AdapterRegistry::new();
        registry.register(mlx_adapter()); // Only TextGeneration

        let config = ModelConfig::new(ModelModality::SpeechToText, ModelFormat::Gguf);
        let err = registry.resolve(&config).unwrap_err();
        assert_eq!(
            err,
            ResolutionError::ModalityNotSupported(ModelModality::SpeechToText)
        );
    }

    #[test]
    fn test_format_not_supported() {
        let mut registry = AdapterRegistry::new();
        registry.register(mlx_adapter()); // Supports Gguf + Safetensors, not CoreMl

        let config = ModelConfig::new(ModelModality::TextGeneration, ModelFormat::CoreMl);
        let err = registry.resolve(&config).unwrap_err();
        assert_eq!(
            err,
            ResolutionError::FormatNotSupported(ModelFormat::CoreMl)
        );
    }

    #[test]
    fn test_quantization_not_supported() {
        let mut registry = AdapterRegistry::new();
        registry.register(llama_cpp_adapter()); // Supports q4_0, q4_1, q8_0

        let config = ModelConfig::new(ModelModality::TextGeneration, ModelFormat::Gguf)
            .with_quantization("f16");
        let err = registry.resolve(&config).unwrap_err();
        assert_eq!(err, ResolutionError::QuantizationNotSupported("f16".into()));
    }

    #[test]
    fn test_no_compatible_adapter_combined() {
        let mut registry = AdapterRegistry::new();
        // whisper only supports SpeechToText + Safetensors
        registry.register(whisper_adapter());
        // cloud supports TextGeneration + Embedding, Gguf + Safetensors
        registry.register(cloud_adapter());

        // Request: SpeechToText + Gguf → whisper has modality but wrong format,
        // cloud has format but wrong modality
        let config = ModelConfig::new(ModelModality::SpeechToText, ModelFormat::Gguf);
        let err = registry.resolve(&config).unwrap_err();
        assert!(matches!(err, ResolutionError::NoCompatibleAdapter { .. }));
    }

    // --- Registry management tests ---

    #[test]
    fn test_register_and_unregister() {
        let mut registry = AdapterRegistry::new();
        registry.register(mlx_adapter());
        assert_eq!(registry.len(), 1);

        let removed = registry.unregister("mlx-local");
        assert!(removed);
        assert_eq!(registry.len(), 0);
        assert!(registry.is_empty());
    }

    #[test]
    fn test_register_overwrites_existing() {
        let mut registry = AdapterRegistry::new();
        registry.register(mlx_adapter());

        // Register a different adapter with the same ID
        let updated = AdapterDescriptor::new("mlx-local", "MLX Backend v2")
            .with_modality(ModelModality::VisionLanguage)
            .with_format(ModelFormat::Safetensors);
        registry.register(updated);

        assert_eq!(registry.len(), 1);
        let desc = registry.get("mlx-local").unwrap();
        assert_eq!(desc.name, "MLX Backend v2");
        assert!(desc.modalities.contains(&ModelModality::VisionLanguage));
    }

    #[test]
    fn test_adapter_ids_sorted() {
        let mut registry = AdapterRegistry::new();
        registry.register(mlx_adapter());
        registry.register(llama_cpp_adapter());
        registry.register(whisper_adapter());

        let ids = registry.adapter_ids();
        assert_eq!(ids, vec!["llama-cpp", "mlx-local", "whisper-mlx"]);
    }

    #[test]
    fn test_resolution_error_display() {
        let err = ResolutionError::EmptyRegistry;
        assert_eq!(format!("{}", err), "adapter registry is empty");

        let err = ResolutionError::ModalityNotSupported(ModelModality::TextToSpeech);
        assert_eq!(
            format!("{}", err),
            "no adapter supports modality: text-to-speech"
        );
    }
}
