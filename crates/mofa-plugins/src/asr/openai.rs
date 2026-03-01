//! OpenAI ASR Backend Implementation
//!
//! Provides an implementation of `ASREngine` that utilizes the OpenAI Whisper
//! API for transcription.

use crate::asr::{ASREngine, ASRPluginConfig};
use mofa_kernel::plugin::{PluginError, PluginResult};
use reqwest::multipart;
use serde::Deserialize;
use std::env;
use tracing::{debug, error, info};

const OPENAI_API_URL: &str = "https://api.openai.com/v1/audio/transcriptions";

/// OpenAI ASR Engine Implementation
pub struct OpenAIASR {
    api_key: String,
    config: ASRPluginConfig,
    client: reqwest::Client,
}

#[derive(Debug, Deserialize)]
struct TranscriptionResponse {
    text: String,
}

impl OpenAIASR {
    /// Create a new OpenAI ASR backend by reading `OPENAI_API_KEY` from the environment.
    pub fn new(config: ASRPluginConfig) -> Result<Self, String> {
        let api_key = env::var("OPENAI_API_KEY")
            .map_err(|_| "OPENAI_API_KEY environment variable not set".to_string())?;

        Ok(Self {
            api_key,
            config,
            client: reqwest::Client::new(),
        })
    }

    /// Create with an explicit API key
    pub fn with_api_key(api_key: impl Into<String>, config: ASRPluginConfig) -> Self {
        Self {
            api_key: api_key.into(),
            config,
            client: reqwest::Client::new(),
        }
    }
}

#[async_trait::async_trait]
impl ASREngine for OpenAIASR {
    async fn transcribe(&self, audio: &[u8], language: Option<&str>) -> PluginResult<String> {
        debug!(
            "Sending {} bytes of audio to OpenAI Whisper API",
            audio.len()
        );

        // Create the multipart form
        let file_part = multipart::Part::bytes(audio.to_vec())
            .file_name("audio.wav")
            .mime_str("audio/wav")
            .map_err(|e| {
                PluginError::ExecutionFailed(format!("Failed to build multipart chunk: {}", e))
            })?;

        let model = self.config.default_model.clone();

        let mut form = multipart::Form::new()
            .part("file", file_part)
            .text("model", model);

        if let Some(l) = language.or(Some(&self.config.default_language)) {
            form = form.text("language", l.to_string());
        }

        // Execute Request
        let res = self
            .client
            .post(OPENAI_API_URL)
            .bearer_auth(&self.api_key)
            .multipart(form)
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

        let resp: TranscriptionResponse = res.json().await.map_err(|e| {
            PluginError::ExecutionFailed(format!("Failed to parse JSON response: {}", e))
        })?;

        info!("Successfully received transcription from OpenAI Whisper");
        Ok(resp.text)
    }

    fn name(&self) -> &str {
        "OpenAIASR"
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}
