//! OpenAI TTS Backend Implementation
//!
//! Provides an implementation of `TTSEngine` that utilizes the OpenAI audio
//! API for text-to-speech synthesis (e.g. tts-1).

use crate::tts::{TTSEngine, TTSPluginConfig, VoiceInfo};
use mofa_kernel::plugin::{PluginError, PluginResult};
use serde::{Deserialize, Serialize};
use std::env;
use tracing::{debug, error, info};

const OPENAI_API_URL: &str = "https://api.openai.com/v1/audio/speech";

/// OpenAI TTS Engine Implementation
pub struct OpenAITTS {
    api_key: String,
    config: TTSPluginConfig,
    client: reqwest::Client,
    voices: Vec<VoiceInfo>,
}

#[derive(Debug, Serialize)]
struct TtsRequest<'a> {
    model: &'a str,
    input: &'a str,
    voice: &'a str,
    response_format: &'a str,
}

impl OpenAITTS {
    /// Create a new OpenAI TTS backend by reading `OPENAI_API_KEY` from the environment.
    pub fn new(config: TTSPluginConfig) -> Result<Self, String> {
        let api_key = env::var("OPENAI_API_KEY")
            .map_err(|_| "OPENAI_API_KEY environment variable not set".to_string())?;

        Ok(Self::with_api_key(api_key, config))
    }

    /// Create with an explicit API key
    pub fn with_api_key(api_key: impl Into<String>, config: TTSPluginConfig) -> Self {
        // OpenAI standard voices
        let voices = vec![
            VoiceInfo::new("alloy", "Alloy (Neutral)", "en"),
            VoiceInfo::new("echo", "Echo (Neutral)", "en"),
            VoiceInfo::new("fable", "Fable (British/Happy)", "en"),
            VoiceInfo::new("onyx", "Onyx (Deep/Authoritative)", "en"),
            VoiceInfo::new("nova", "Nova (Energetic/Female)", "en"),
            VoiceInfo::new("shimmer", "Shimmer (Clear/Female)", "en"),
        ];

        Self {
            api_key: api_key.into(),
            config,
            client: reqwest::Client::new(),
            voices,
        }
    }
}

#[async_trait::async_trait]
impl TTSEngine for OpenAITTS {
    async fn synthesize(&self, text: &str, voice: &str) -> PluginResult<Vec<u8>> {
        debug!(
            "Synthesizing {} chars with OpenAI TTS (model: {}, voice: {})",
            text.len(),
            self.config.model_version,
            voice
        );

        // Define default model if "v1.1" (from kokoro config) is used
        let model = if self.config.model_version.starts_with('v') {
            "tts-1"
        } else {
            &self.config.model_version
        };

        let req_body = TtsRequest {
            model,
            input: text,
            voice,
            response_format: "wav",
        };

        let res = self
            .client
            .post(OPENAI_API_URL)
            .bearer_auth(&self.api_key)
            .json(&req_body)
            .send()
            .await
            .map_err(|e| PluginError::ExecutionFailed(format!("HTTP request failed: {}", e)))?;

        if !res.status().is_success() {
            let status = res.status();
            let body = res.text().await.unwrap_or_default();
            error!("OpenAI API error: {} - {}", status, body);
            return Err(PluginError::ExecutionFailed(format!(
                "OpenAI API error {}: {}",
                status, body
            )));
        }

        let audio_bytes = res.bytes().await.map_err(|e| {
            PluginError::ExecutionFailed(format!("Failed to read response bytes: {}", e))
        })?;

        info!("Successfully received synthesized WAV audio from OpenAI");
        Ok(audio_bytes.to_vec())
    }

    async fn synthesize_stream(
        &self,
        text: &str,
        voice: &str,
        callback: Box<dyn Fn(Vec<u8>) + Send + Sync>,
    ) -> PluginResult<()> {
        debug!(
            "Stream synthesizing {} chars with OpenAI TTS (model: {}, voice: {})",
            text.len(),
            self.config.model_version,
            voice
        );

        // OpenAI's standard TTS API currently doesn't support real chunked streaming output
        // where it yields byte-by-byte. So we synthesize the whole block and call the callback once.
        // (Text streaming chunking should be driven by the caller passing smaller texts in).
        let audio = self.synthesize(text, voice).await?;
        callback(audio);
        Ok(())
    }

    async fn list_voices(&self) -> PluginResult<Vec<VoiceInfo>> {
        Ok(self.voices.clone())
    }

    fn name(&self) -> &str {
        "OpenAITTS"
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}
