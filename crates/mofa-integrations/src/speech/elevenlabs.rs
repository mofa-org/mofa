//! ElevenLabs TTS adapter implementation.

use async_trait::async_trait;
use mofa_kernel::agent::{AgentError, AgentResult};
use mofa_kernel::speech::{AudioFormat, AudioOutput, TtsAdapter, TtsConfig, VoiceDescriptor};
use reqwest::Client;
use serde::Deserialize;
use std::time::Duration;
use tracing::{debug, error};

// ============================================================================
// Types
// ============================================================================

/// Configuration for ElevenLabs TTS.
#[derive(Debug, Clone)]
pub struct ElevenLabsConfig {
    pub api_key: Option<String>,
    pub base_url: String,
    pub model_id: String,
    pub timeout: Duration,
    pub max_retries: usize,
}

impl Default for ElevenLabsConfig {
    fn default() -> Self {
        Self {
            api_key: std::env::var("ELEVENLABS_API_KEY").ok(),
            base_url: "https://api.elevenlabs.io/v1".to_string(),
            model_id: "eleven_monolingual_v1".to_string(),
            timeout: Duration::from_secs(30),
            max_retries: 3,
        }
    }
}

impl ElevenLabsConfig {
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
            .or_else(|| std::env::var("ELEVENLABS_API_KEY").ok())
            .ok_or_else(|| AgentError::ConfigError("Missing ELEVENLABS_API_KEY".to_string()))
    }
}

#[derive(Debug, Deserialize)]
struct ElevenLabsVoicesResponse {
    voices: Vec<ElevenLabsVoice>,
}

#[derive(Debug, Deserialize)]
struct ElevenLabsVoice {
    voice_id: String,
    name: String,
    labels: std::collections::HashMap<String, String>,
    preview_url: Option<String>,
}

// ============================================================================
// Adapter
// ============================================================================

/// ElevenLabs TTS adapter.
pub struct ElevenLabsTtsAdapter {
    config: ElevenLabsConfig,
    client: Client,
}

impl ElevenLabsTtsAdapter {
    pub fn new(config: ElevenLabsConfig) -> Self {
        Self {
            config,
            client: Client::new(),
        }
    }
}

#[async_trait]
impl TtsAdapter for ElevenLabsTtsAdapter {
    fn name(&self) -> &str {
        "elevenlabs"
    }

    async fn synthesize(
        &self,
        text: &str,
        voice: &str,
        _config: &TtsConfig,
    ) -> AgentResult<AudioOutput> {
        let api_key = self.config.resolve_api_key()?;
        let url = format!("{}/text-to-speech/{}", self.config.base_url, voice);

        let payload = serde_json::json!({
            "text": text,
            "model_id": self.config.model_id,
            "voice_settings": {
                "stability": 0.5,
                "similarity_boost": 0.5,
            }
        });

        debug!("[elevenlabs] synthesizing text of length {}", text.len());

        let mut last_error = None;
        for attempt in 0..=self.config.max_retries {
            if attempt > 0 {
                tokio::time::sleep(Duration::from_millis(500 * attempt as u64)).await;
            }

            let response = self
                .client
                .post(&url)
                .header("xi-api-key", &api_key)
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
                    return Ok(AudioOutput::new(data.to_vec(), AudioFormat::Mp3, 44100));
                }
                Ok(resp) => {
                    let status = resp.status();
                    let err_body = resp.text().await.unwrap_or_default();
                    error!("[elevenlabs] API error: {} - {}", status, err_body);
                    last_error = Some(AgentError::Other(format!(
                        "ElevenLabs error {}: {}",
                        status, err_body
                    )));
                    if status.as_u16() != 429 && !status.is_server_error() {
                        break;
                    }
                }
                Err(e) => {
                    error!("[elevenlabs] request failed: {}", e);
                    last_error = Some(AgentError::Other(e.to_string()));
                }
            }
        }

        Err(last_error.unwrap_or_else(|| AgentError::Other("Unknown error".to_string())))
    }

    async fn list_voices(&self) -> AgentResult<Vec<VoiceDescriptor>> {
        let api_key = self.config.resolve_api_key()?;
        let url = format!("{}/voices", self.config.base_url);

        let response = self
            .client
            .get(&url)
            .header("xi-api-key", &api_key)
            .send()
            .await
            .map_err(|e| AgentError::Other(e.to_string()))?;

        if !response.status().is_success() {
            return Err(AgentError::Other(format!(
                "ElevenLabs list_voices failed: {}",
                response.status()
            )));
        }

        let body: ElevenLabsVoicesResponse = response
            .json()
            .await
            .map_err(|e| AgentError::Other(e.to_string()))?;

        Ok(body
            .voices
            .into_iter()
            .map(|v| {
                VoiceDescriptor {
                    id: v.voice_id,
                    name: v.name,
                    language: "en".to_string(), // ElevenLabs voices are often multilingual
                    gender: v.labels.get("gender").cloned(),
                    preview_url: v.preview_url,
                }
            })
            .collect())
    }

    async fn health_check(&self) -> AgentResult<bool> {
        let _ = self.config.resolve_api_key()?;
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
    fn config_default() {
        let cfg = ElevenLabsConfig::new();
        assert_eq!(cfg.base_url, "https://api.elevenlabs.io/v1");
    }

    #[test]
    fn elevenlabs_response_deserialize() {
        let json = r#"{
            "voices": [
                {
                    "voice_id": "v1",
                    "name": "Voice 1",
                    "labels": {"gender": "female"},
                    "preview_url": "http://example.com/p1"
                }
            ]
        }"#;
        let resp: ElevenLabsVoicesResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.voices.len(), 1);
        assert_eq!(resp.voices[0].name, "Voice 1");
    }
}
