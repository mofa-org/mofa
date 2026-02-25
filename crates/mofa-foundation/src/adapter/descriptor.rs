//! Adapter descriptors and capabilities.
//!
//! This module defines the types used to describe the capabilities of a model adapter,
//! such as the supported modalities (text, vision, audio) and model file formats
//! (GGUF, Safetensors, PyTorch).

use std::collections::HashSet;
use std::fmt;

/// Defines the operational modality of a model.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ModelModality {
    /// Text-to-text generation (e.g., standard LLMs)
    TextGeneration,
    /// Vision-language understanding (e.g., LLaVA)
    VisionLanguage,
    /// Automatic Speech Recognition (Audio-to-text)
    SpeechToText,
    /// Text-to-speech generation
    TextToSpeech,
    /// Text embeddings
    Embedding,
}

impl fmt::Display for ModelModality {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TextGeneration => write!(f, "text-generation"),
            Self::VisionLanguage => write!(f, "vision-language"),
            Self::SpeechToText => write!(f, "speech-to-text"),
            Self::TextToSpeech => write!(f, "text-to-speech"),
            Self::Embedding => write!(f, "embedding"),
        }
    }
}

/// Defines the underlying weight format of a model.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ModelFormat {
    /// GGUF (used by llama.cpp)
    Gguf,
    /// Safetensors (standard Hugging Face format)
    Safetensors,
    /// Traditional PyTorch `bin` / `pt`
    PyTorchCheckpoint,
    /// Apple CoreML
    CoreMl,
    /// ONNX Runtime format
    Onnx,
}

impl fmt::Display for ModelFormat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Gguf => write!(f, "gguf"),
            Self::Safetensors => write!(f, "safetensors"),
            Self::PyTorchCheckpoint => write!(f, "pytorch"),
            Self::CoreMl => write!(f, "coreml"),
            Self::Onnx => write!(f, "onnx"),
        }
    }
}

/// Describes the capabilities and constraints of a registered model adapter.
///
/// Each adapter declares what modalities, formats, and quantization profiles
/// it supports. The [`AdapterRegistry`](super::AdapterRegistry) uses this
/// information to deterministically resolve the best adapter for a given request.
#[derive(Debug, Clone)]
pub struct AdapterDescriptor {
    /// Unique identifier for this adapter (e.g., "mlx-local", "llama-cpp")
    pub id: String,

    /// Human-readable name
    pub name: String,

    /// Modalities this adapter supports
    pub modalities: HashSet<ModelModality>,

    /// File formats this adapter can load
    pub supported_formats: HashSet<ModelFormat>,

    /// Quantization profiles supported (e.g., "q4_0", "q8_0", "f16").
    /// An empty set implies no specific quantization constraints (e.g., cloud API).
    pub supported_quantization: HashSet<String>,
}

impl AdapterDescriptor {
    /// Creates a new `AdapterDescriptor`.
    pub fn new(id: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            modalities: HashSet::new(),
            supported_formats: HashSet::new(),
            supported_quantization: HashSet::new(),
        }
    }

    /// Builder: add a supported modality.
    pub fn with_modality(mut self, modality: ModelModality) -> Self {
        self.modalities.insert(modality);
        self
    }

    /// Builder: add multiple supported modalities.
    pub fn with_modalities(mut self, modalities: impl IntoIterator<Item = ModelModality>) -> Self {
        self.modalities.extend(modalities);
        self
    }

    /// Builder: add a supported format.
    pub fn with_format(mut self, format: ModelFormat) -> Self {
        self.supported_formats.insert(format);
        self
    }

    /// Builder: add multiple supported formats.
    pub fn with_formats(mut self, formats: impl IntoIterator<Item = ModelFormat>) -> Self {
        self.supported_formats.extend(formats);
        self
    }

    /// Builder: add a supported quantization profile string.
    pub fn with_quantization(mut self, quant: impl Into<String>) -> Self {
        self.supported_quantization.insert(quant.into());
        self
    }
}
