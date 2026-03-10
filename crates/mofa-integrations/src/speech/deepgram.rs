//! Deepgram ASR adapter implementation.

use async_trait::async_trait;
use mofa_kernel::agent::{AgentError, AgentResult};
use mofa_kernel::speech::{AsrAdapter, AsrConfig, TranscriptionResult, TranscriptionSegment};
use reqwest::Client;
use serde::Deserialize;
use std::time::Duration;
use tracing::{error, info};

// ============================================================================
// Types
// ============================================================================

/// Configuration for Deepgram ASR.
#[derive(Debug, Clone)]
pub struct DeepgramConfig {
    pub api_key: Option<String>,
    pub base_url: String,
    pub model: String,
    pub timeout: Duration,
    pub max_retries: usize,
}

impl Default for DeepgramConfig {
    fn default() -> Self {
        Self {
            api_key: std::env::var("DEEPGRAM_API_KEY").ok(),
            base_url: "https://api.deepgram.com/v1".to_string(),
            model: "nova-2".to_string(),
            timeout: Duration::from_secs(30),
            max_retries: 3,
        }
    }
}

impl DeepgramConfig {
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
            .or_else(|| std::env::var("DEEPGRAM_API_KEY").ok())
            .ok_or_else(|| AgentError::ConfigError("Missing DEEPGRAM_API_KEY".to_string()))
    }
}

#[derive(Debug, Deserialize)]
struct DeepgramResponse {
    results: DeepgramResults,
    metadata: DeepgramMetadata,
}

#[derive(Debug, Deserialize)]
struct DeepgramResults {
    channels: Vec<DeepgramChannel>,
}

#[derive(Debug, Deserialize)]
struct DeepgramChannel {
    alternatives: Vec<DeepgramAlternative>,
}

#[derive(Debug, Deserialize)]
struct DeepgramAlternative {
    transcript: String,
    confidence: f32,
    words: Option<Vec<DeepgramWord>>,
}

#[derive(Debug, Deserialize)]
struct DeepgramWord {
    word: String,
    start: f32,
    end: f32,
}

#[derive(Debug, Deserialize)]
struct DeepgramMetadata {
    language: String,
}

// ============================================================================
// Adapter
// ============================================================================

/// Deepgram ASR adapter.
pub struct DeepgramAsrAdapter {
    config: DeepgramConfig,
    client: Client,
}

impl DeepgramAsrAdapter {
    pub fn new(config: DeepgramConfig) -> Self {
        Self {
            config,
            client: Client::new(),
        }
    }
}

#[async_trait]
impl AsrAdapter for DeepgramAsrAdapter {
    fn name(&self) -> &str {
        "deepgram"
    }

    async fn transcribe(
        &self,
        audio: &[u8],
        config: &AsrConfig,
    ) -> AgentResult<TranscriptionResult> {
        let api_key = self.config.resolve_api_key()?;
        let mut url = format!(
            "{}/listen?model={}&smart_format=true",
            self.config.base_url, self.config.model
        );

        if let Some(lang) = &config.language {
            url.push_str(&format!("&language={}", lang));
        }
        if config.timestamps {
            url.push_str("&utterances=true&diarize=false");
        }

        info!("[deepgram] transcribing {} bytes", audio.len());

        let mut last_error = None;
        for attempt in 0..=self.config.max_retries {
            if attempt > 0 {
                tokio::time::sleep(Duration::from_millis(500 * attempt as u64)).await;
            }

            let response = self
                .client
                .post(&url)
                .header("Authorization", format!("Token {}", api_key))
                .header("Content-Type", "application/octet-stream") // Deepgram handles most formats automatically
                .body(audio.to_vec())
                .timeout(self.config.timeout)
                .send()
                .await;

            match response {
                Ok(resp) if resp.status().is_success() => {
                    let body: DeepgramResponse = resp
                        .json()
                        .await
                        .map_err(|e| AgentError::Other(e.to_string()))?;
                    let alt = body
                        .results
                        .channels
                        .get(0)
                        .and_then(|ch| ch.alternatives.get(0))
                        .ok_or_else(|| {
                            AgentError::Other(
                                "Deepgram response missing transcription alternatives".to_string(),
                            )
                        })?;

                    let segments = alt.words.as_ref().map(|words| {
                        words
                            .iter()
                            .map(|w| TranscriptionSegment {
                                text: w.word.clone(),
                                start: w.start,
                                end: w.end,
                            })
                            .collect()
                    });

                    return Ok(TranscriptionResult {
                        text: alt.transcript.clone(),
                        language: Some(body.metadata.language),
                        confidence: Some(alt.confidence),
                        segments,
                    });
                }
                Ok(resp) => {
                    let status = resp.status();
                    let err_body = resp.text().await.unwrap_or_default();
                    error!("[deepgram] API error: {} - {}", status, err_body);
                    last_error = Some(AgentError::Other(format!(
                        "Deepgram error {}: {}",
                        status, err_body
                    )));
                    if status.as_u16() != 429 && !status.is_server_error() {
                        break;
                    }
                }
                Err(e) => {
                    error!("[deepgram] request failed: {}", e);
                    last_error = Some(AgentError::Other(e.to_string()));
                }
            }
        }

        Err(last_error.unwrap_or_else(|| AgentError::Other("Unknown error".to_string())))
    }

    fn supported_languages(&self) -> Vec<String> {
        // Deepgram supports 34+ languages
        vec![
            "en", "zh", "fr", "de", "hi", "it", "ja", "ko", "pt", "ru", "es", "tr", "vi",
        ]
        .into_iter()
        .map(|s| s.to_string())
        .collect()
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
        let cfg = DeepgramConfig::new();
        assert_eq!(cfg.model, "nova-2");
    }

    #[test]
    fn deepgram_response_deserialize() {
        let json = r#"{
            "metadata": {"language": "en"},
            "results": {
                "channels": [
                    {
                        "alternatives": [
                            {
                                "transcript": "hello world",
                                "confidence": 0.99,
                                "words": [{"word": "hello", "start": 0.0, "end": 0.5}]
                            }
                        ]
                    }
                ]
            }
        }"#;
        let resp: DeepgramResponse = serde_json::from_str(json).unwrap();
        assert_eq!(
            resp.results.channels[0].alternatives[0].transcript,
            "hello world"
        );
    }
}
