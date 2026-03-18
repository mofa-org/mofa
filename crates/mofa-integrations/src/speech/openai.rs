//! OpenAI speech adapters for TTS-1 and Whisper ASR.

use async_trait::async_trait;
use mofa_kernel::agent::{AgentError, AgentResult};
use mofa_kernel::speech::{
    AsrAdapter, AsrConfig, AudioFormat, AudioOutput, TranscriptionResult, TtsAdapter, TtsConfig,
    VoiceDescriptor,
};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::time::Duration;
use tracing::{debug, error, info};

// ============================================================================
// Configuration
// ============================================================================

/// Configuration for OpenAI speech models.
#[derive(Debug, Clone)]
pub struct OpenAiSpeechConfig {
    pub api_key: Option<String>,
    pub base_url: String,
    pub timeout: Duration,
    pub max_retries: usize,
}

impl Default for OpenAiSpeechConfig {
    fn default() -> Self {
        Self {
            api_key: std::env::var("OPENAI_API_KEY").ok(),
            base_url: "https://api.openai.com/v1".to_string(),
            timeout: Duration::from_secs(30),
            max_retries: 3,
        }
    }
}

impl OpenAiSpeechConfig {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_api_key(mut self, key: impl Into<String>) -> Self {
        self.api_key = Some(key.into());
        self
    }

    fn resolve_api_key(&self) -> AgentResult<String> {
        self.api_key
            .clone()
            .or_else(|| std::env::var("OPENAI_API_KEY").ok())
            .ok_or_else(|| AgentError::ConfigError("Missing OPENAI_API_KEY".to_string()))
    }
}

// ============================================================================
// Text-to-Speech (TTS-1)
// ============================================================================

/// OpenAI TTS models.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum OpenAiTtsModel {
    #[serde(rename = "tts-1")]
    Tts1,
    #[serde(rename = "tts-1-hd")]
    Tts1Hd,
}

impl fmt::Display for OpenAiTtsModel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Tts1 => write!(f, "tts-1"),
            Self::Tts1Hd => write!(f, "tts-1-hd"),
        }
    }
}

/// OpenAI TTS adapter implementation.
pub struct OpenAiTtsAdapter {
    config: OpenAiSpeechConfig,
    model: OpenAiTtsModel,
    client: Client,
}

impl OpenAiTtsAdapter {
    pub fn new(config: OpenAiSpeechConfig) -> Self {
        Self {
            config,
            model: OpenAiTtsModel::Tts1,
            client: Client::new(),
        }
    }

    pub fn with_model(mut self, model: OpenAiTtsModel) -> Self {
        self.model = model;
        self
    }
}

#[async_trait]
impl TtsAdapter for OpenAiTtsAdapter {
    fn name(&self) -> &str {
        "openai-tts"
    }

    async fn synthesize(
        &self,
        text: &str,
        voice: &str,
        config: &TtsConfig,
    ) -> AgentResult<AudioOutput> {
        let api_key = self.config.resolve_api_key()?;
        let url = format!("{}/audio/speech", self.config.base_url);

        let format = config.format.unwrap_or(AudioFormat::Mp3);
        let speed = config.speed.unwrap_or(1.0);

        let payload = serde_json::json!({
            "model": self.model,
            "input": text,
            "voice": voice,
            "response_format": format.to_string(),
            "speed": speed,
        });

        debug!(
            "[openai-tts] synthesizing text: \"{}\" with voice={}",
            text, voice
        );

        let mut last_error = None;
        for attempt in 0..=self.config.max_retries {
            if attempt > 0 {
                info!(
                    "[openai-tts] retry attempt {}/{}",
                    attempt, self.config.max_retries
                );
                tokio::time::sleep(Duration::from_millis(500 * attempt as u64)).await;
            }

            let response = self
                .client
                .post(&url)
                .bearer_auth(&api_key)
                .json(&payload)
                .timeout(self.config.timeout)
                .send()
                .await;

            match response {
                Ok(resp) if resp.status().is_success() => {
                    let data = resp
                        .bytes()
                        .await
                        .map_err(|e| AgentError::Other(e.to_string()))?;
                    return Ok(AudioOutput::new(
                        data.to_vec(),
                        format,
                        config.sample_rate.unwrap_or(24000),
                    ));
                }
                Ok(resp) => {
                    let status = resp.status();
                    let err_body = resp.text().await.unwrap_or_default();
                    error!("[openai-tts] API error: {} - {}", status, err_body);
                    last_error = Some(AgentError::Other(format!(
                        "OpenAI TTS error {}: {}",
                        status, err_body
                    )));
                    if status.as_u16() != 429 && !status.is_server_error() {
                        break;
                    }
                }
                Err(e) => {
                    error!("[openai-tts] request failed: {}", e);
                    last_error = Some(AgentError::Other(e.to_string()));
                }
            }
        }

        Err(last_error.unwrap_or_else(|| AgentError::Other("Unknown error".to_string())))
    }

    async fn list_voices(&self) -> AgentResult<Vec<VoiceDescriptor>> {
        // OpenAI TTS has fixed voices
        Ok(vec![
            VoiceDescriptor {
                id: "alloy".to_string(),
                name: "Alloy".to_string(),
                language: "en".to_string(),
                gender: Some("Neutral".to_string()),
                preview_url: None,
            },
            VoiceDescriptor {
                id: "echo".to_string(),
                name: "Echo".to_string(),
                language: "en".to_string(),
                gender: Some("Male".to_string()),
                preview_url: None,
            },
            VoiceDescriptor {
                id: "fable".to_string(),
                name: "Fable".to_string(),
                language: "en".to_string(),
                gender: Some("Neutral".to_string()),
                preview_url: None,
            },
            VoiceDescriptor {
                id: "onyx".to_string(),
                name: "Onyx".to_string(),
                language: "en".to_string(),
                gender: Some("Male".to_string()),
                preview_url: None,
            },
            VoiceDescriptor {
                id: "nova".to_string(),
                name: "Nova".to_string(),
                language: "en".to_string(),
                gender: Some("Female".to_string()),
                preview_url: None,
            },
            VoiceDescriptor {
                id: "shimmer".to_string(),
                name: "Shimmer".to_string(),
                language: "en".to_string(),
                gender: Some("Female".to_string()),
                preview_url: None,
            },
        ])
    }

    async fn health_check(&self) -> AgentResult<bool> {
        let _ = self.config.resolve_api_key()?;
        Ok(true)
    }
}

// ============================================================================
// Automated Speech Recognition (Whisper)
// ============================================================================

/// OpenAI ASR (Whisper) adapter implementation.
pub struct OpenAiAsrAdapter {
    config: OpenAiSpeechConfig,
    client: Client,
}

impl OpenAiAsrAdapter {
    pub fn new(config: OpenAiSpeechConfig) -> Self {
        Self {
            config,
            client: Client::new(),
        }
    }
}

#[async_trait]
impl AsrAdapter for OpenAiAsrAdapter {
    fn name(&self) -> &str {
        "openai-whisper"
    }

    async fn transcribe(
        &self,
        audio: &[u8],
        config: &AsrConfig,
    ) -> AgentResult<TranscriptionResult> {
        let api_key = self.config.resolve_api_key()?;
        let url = format!("{}/audio/transcriptions", self.config.base_url);

        let mut last_error = None;
        for attempt in 0..=self.config.max_retries {
            if attempt > 0 {
                info!(
                    "[openai-whisper] retry attempt {}/{}",
                    attempt, self.config.max_retries
                );
                tokio::time::sleep(Duration::from_millis(500 * attempt as u64)).await;
            }

            // Build form per attempt because reqwest::multipart::Form is not Clone
            let mut form = reqwest::multipart::Form::new()
                .part(
                    "file",
                    reqwest::multipart::Part::bytes(audio.to_vec())
                        .file_name("audio")
                        .mime_str("application/octet-stream")
                        .map_err(|e| AgentError::Other(e.to_string()))?,
                )
                .text("model", "whisper-1");

            if let Some(lang) = &config.language {
                form = form.text("language", lang.clone());
            }
            if let Some(prompt) = &config.prompt {
                form = form.text("prompt", prompt.clone());
            }

            let response = self
                .client
                .post(&url)
                .bearer_auth(&api_key)
                .multipart(form)
                .timeout(self.config.timeout)
                .send()
                .await;

            match response {
                Ok(resp) if resp.status().is_success() => {
                    let json: serde_json::Value = resp
                        .json()
                        .await
                        .map_err(|e| AgentError::Other(e.to_string()))?;
                    let text = json["text"].as_str().unwrap_or_default().to_string();
                    let language = json["language"].as_str().map(|s| s.to_string());
                    return Ok(TranscriptionResult {
                        text,
                        language,
                        confidence: None,
                        segments: None,
                    });
                }
                Ok(resp) => {
                    let status = resp.status();
                    let err_body = resp.text().await.unwrap_or_default();
                    error!("[openai-whisper] API error: {} - {}", status, err_body);
                    last_error = Some(AgentError::Other(format!(
                        "OpenAI Whisper error {}: {}",
                        status, err_body
                    )));
                    if status.as_u16() != 429 && !status.is_server_error() {
                        break;
                    }
                }
                Err(e) => {
                    error!("[openai-whisper] request failed: {}", e);
                    last_error = Some(AgentError::Other(e.to_string()));
                }
            }
        }

        Err(last_error.unwrap_or_else(|| AgentError::Other("Unknown error".to_string())))
    }

    fn supported_languages(&self) -> Vec<String> {
        [
            "en", "zh", "de", "es", "ru", "ko", "fr", "ja", "pt", "tr", "pl", "ca", "nl", "ar",
            "sv", "it", "id", "hi", "fi", "vi", "he", "uk", "el", "ms", "cs", "ro", "da", "hu",
            "ta", "no", "th", "ur", "hr", "bg", "lt", "la", "mi", "ml", "cy", "sk", "te", "fa",
            "lv", "bn", "sr", "az", "sl", "kn", "et", "mk", "br", "eu", "is", "hy", "ne", "mn",
            "bs", "kk", "sq", "sw", "gl", "mr", "pa", "si", "km", "sn", "yo", "so", "af", "oc",
            "ka", "be", "tg", "sd", "gu", "am", "yi", "lo", "uz", "fo", "ht", "ps", "tk", "nn",
            "mt", "sa", "lb", "my", "ba", "as", "tt", "haw", "ln", "ha", "mg", "jw", "su",
        ]
        .iter()
        .map(|s| s.to_string())
        .collect()
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_default() {
        let cfg = OpenAiSpeechConfig::new();
        assert_eq!(cfg.base_url, "https://api.openai.com/v1");
        assert_eq!(cfg.max_retries, 3);
    }

    #[test]
    fn tts_model_display() {
        assert_eq!(OpenAiTtsModel::Tts1.to_string(), "tts-1");
        assert_eq!(OpenAiTtsModel::Tts1Hd.to_string(), "tts-1-hd");
    }

    #[tokio::test]
    async fn tts_list_voices() {
        let adapter = OpenAiTtsAdapter::new(OpenAiSpeechConfig::new());
        let voices = adapter.list_voices().await.unwrap();
        assert_eq!(voices.len(), 6);
        assert_eq!(voices[0].id, "alloy");
    }

    #[test]
    fn asr_supported_languages() {
        let adapter = OpenAiAsrAdapter::new(OpenAiSpeechConfig::new());
        let langs = adapter.supported_languages();
        assert!(langs.contains(&"en".to_string()));
        assert!(langs.contains(&"zh".to_string()));
    }
}
