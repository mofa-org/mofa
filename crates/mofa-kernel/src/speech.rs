//! Kernel trait contracts for Text-to-Speech (TTS) and
//! Automated Speech Recognition (ASR).
//!
//! These traits provide a provider-agnostic interface for speech services,
//! allowing the MoFA foundation and plugins to interact with different
//! cloud or local speech engines.

use crate::agent::AgentResult;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::fmt;

// ============================================================================
// Common Types
// ============================================================================

/// Supported audio formats for speech processing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
#[non_exhaustive]
pub enum AudioFormat {
    Wav,
    Mp3,
    Pcm,
    Opus,
    Aac,
    Flac,
}

impl fmt::Display for AudioFormat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            Self::Wav => "wav",
            Self::Mp3 => "mp3",
            Self::Pcm => "pcm",
            Self::Opus => "opus",
            Self::Aac => "aac",
            Self::Flac => "flac",
        };
        write!(f, "{}", s)
    }
}

/// Generic container for audio data.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioOutput {
    /// Raw audio bytes.
    pub data: Vec<u8>,
    /// Format of the audio data.
    pub format: AudioFormat,
    /// Sample rate in Hz (e.g. 24000, 44100).
    pub sample_rate: u32,
}

impl AudioOutput {
    pub fn new(data: Vec<u8>, format: AudioFormat, sample_rate: u32) -> Self {
        Self {
            data,
            format,
            sample_rate,
        }
    }
}

/// Metadata about a specific voice.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VoiceDescriptor {
    /// Provider-specific ID (e.g. "alloy", "en-US-Standard-A").
    pub id: String,
    /// Human-readable name (e.g. "Alloy", "Jenny").
    pub name: String,
    /// BCP-47 language code (e.g. "en-US", "zh-CN").
    pub language: String,
    /// Optional gender (e.g. "Male", "Female").
    pub gender: Option<String>,
    /// Optional preview URL for the voice sample.
    pub preview_url: Option<String>,
}

impl VoiceDescriptor {
    pub fn new(
        id: impl Into<String>,
        name: impl Into<String>,
        language: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            language: language.into(),
            gender: None,
            preview_url: None,
        }
    }
}

// ============================================================================
// Text-to-Speech (TTS)
// ============================================================================

/// Configuration for TTS synthesis.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TtsConfig {
    /// Target sample rate (Hz).
    pub sample_rate: Option<u32>,
    /// Output audio format.
    pub format: Option<AudioFormat>,
    /// Speaking rate (speed).
    pub speed: Option<f32>,
    /// Speaking pitch.
    pub pitch: Option<f32>,
    /// Language override.
    pub language: Option<String>,
}

impl TtsConfig {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_format(mut self, format: AudioFormat) -> Self {
        self.format = Some(format);
        self
    }

    pub fn with_speed(mut self, speed: f32) -> Self {
        self.speed = Some(speed);
        self
    }
}

/// Trait for Text-to-Speech adapters.
#[async_trait]
pub trait TtsAdapter: Send + Sync {
    /// Get the unique name of this adapter (e.g. "openai-tts").
    fn name(&self) -> &str;

    /// Synthesize text into audio bytes.
    async fn synthesize(
        &self,
        text: &str,
        voice: &str,
        config: &TtsConfig,
    ) -> AgentResult<AudioOutput>;

    /// List available voices for this adapter.
    async fn list_voices(&self) -> AgentResult<Vec<VoiceDescriptor>>;

    /// Verify connectivity and authentication with the provider.
    async fn health_check(&self) -> AgentResult<bool> {
        Ok(true)
    }
}

// ============================================================================
// Automated Speech Recognition (ASR)
// ============================================================================

/// Result of an ASR transcription.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranscriptionResult {
    /// Full transcribed text.
    pub text: String,
    /// Detected language (BCP-47).
    pub language: Option<String>,
    /// Confidence score (0.0 to 1.0).
    pub confidence: Option<f32>,
    /// Word-level or segment-level timestamps.
    pub segments: Option<Vec<TranscriptionSegment>>,
}

impl TranscriptionResult {
    pub fn text_only(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            language: None,
            confidence: None,
            segments: None,
        }
    }
}

/// A segment of transcribed audio with timing information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranscriptionSegment {
    pub text: String,
    pub start: f32,
    pub end: f32,
}

/// Configuration for ASR transcription.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AsrConfig {
    /// Language hint (BCP-47).
    pub language: Option<String>,
    /// Prompt to guide the transcription.
    pub prompt: Option<String>,
    /// Whether to include timestamps.
    pub timestamps: bool,
}

impl AsrConfig {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_language(mut self, lang: impl Into<String>) -> Self {
        self.language = Some(lang.into());
        self
    }
}

/// Trait for Automated Speech Recognition adapters.
#[async_trait]
pub trait AsrAdapter: Send + Sync {
    /// Get the unique name of this adapter (e.g. "openai-whisper").
    fn name(&self) -> &str;

    /// Transcribe audio bytes into text.
    async fn transcribe(
        &self,
        audio: &[u8],
        config: &AsrConfig,
    ) -> AgentResult<TranscriptionResult>;

    /// List supported languages for this adapter.
    fn supported_languages(&self) -> Vec<String> {
        vec![]
    }

    /// Verify connectivity and authentication with the provider.
    async fn health_check(&self) -> AgentResult<bool> {
        Ok(true)
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn audio_format_display() {
        assert_eq!(AudioFormat::Wav.to_string(), "wav");
        assert_eq!(AudioFormat::Mp3.to_string(), "mp3");
    }

    #[test]
    fn tts_config_builder() {
        let cfg = TtsConfig::new()
            .with_format(AudioFormat::Mp3)
            .with_speed(1.5);
        assert_eq!(cfg.format, Some(AudioFormat::Mp3));
        assert_eq!(cfg.speed, Some(1.5));
    }

    #[test]
    fn asr_config_builder() {
        let cfg = AsrConfig::new().with_language("en-US");
        assert_eq!(cfg.language.as_deref(), Some("en-US"));
    }

    #[test]
    fn transcription_result_constructor() {
        let res = TranscriptionResult::text_only("hello world");
        assert_eq!(res.text, "hello world");
    }
}
