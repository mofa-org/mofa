//! Voice Integration Pipeline Traits
//!
//! Provides the core abstractions for chaining ASR, LLM, and TTS models
//! into a unified voice agent pipeline.

use crate::agent::error::AgentResult;
use async_trait::async_trait;
use std::fmt;

/// Input to a voice pipeline stage
#[derive(Debug, Clone, PartialEq)]
pub enum StageInput {
    /// Raw audio waveform data (f32 samples, usually 16kHz or 24kHz)
    Audio(Vec<f32>),
    /// Text transcript or prompt
    Text(String),
}

/// Output from a voice pipeline stage
#[derive(Debug, Clone, PartialEq)]
pub enum StageOutput {
    /// Raw audio waveform data (f32 samples)
    Audio(Vec<f32>),
    /// Text transcript or output
    Text(String),
}

impl fmt::Display for StageInput {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            StageInput::Audio(samples) => write!(f, "Audio({} samples)", samples.len()),
            StageInput::Text(text) => write!(f, "Text({} chars)", text.len()),
        }
    }
}

impl fmt::Display for StageOutput {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            StageOutput::Audio(samples) => write!(f, "Audio({} samples)", samples.len()),
            StageOutput::Text(text) => write!(f, "Text({} chars)", text.len()),
        }
    }
}

impl From<String> for StageInput {
    fn from(text: String) -> Self {
        StageInput::Text(text)
    }
}

impl From<&str> for StageInput {
    fn from(text: &str) -> Self {
        StageInput::Text(text.to_string())
    }
}

impl From<Vec<f32>> for StageInput {
    fn from(samples: Vec<f32>) -> Self {
        StageInput::Audio(samples)
    }
}

impl From<String> for StageOutput {
    fn from(text: String) -> Self {
        StageOutput::Text(text)
    }
}

impl From<Vec<f32>> for StageOutput {
    fn from(samples: Vec<f32>) -> Self {
        StageOutput::Audio(samples)
    }
}

/// A single stage in a Voice Pipeline
///
/// Typical implementations:
/// - ASR Stage: `StageInput::Audio` -> `StageOutput::Text`
/// - LLM Stage: `StageInput::Text` -> `StageOutput::Text`
/// - TTS Stage: `StageInput::Text` -> `StageOutput::Audio`
#[async_trait]
pub trait VoiceStage: Send + Sync {
    /// The unique name or identifier of this stage
    fn name(&self) -> &str;

    /// Process the input and produce output
    async fn process(&self, input: StageInput) -> AgentResult<StageOutput>;
}

/// Configuration for a Voice Pipeline
#[derive(Debug, Clone)]
pub struct VoicePipelineConfig {
    /// Whether to abort the entire pipeline if one stage fails
    pub abort_on_error: bool,
    /// Maximum time in milliseconds allowed for the entire pipeline
    pub timeout_ms: Option<u64>,
}

impl Default for VoicePipelineConfig {
    fn default() -> Self {
        Self {
            abort_on_error: true,
            timeout_ms: Some(30000), // 30 seconds default timeout
        }
    }
}
