//! Audio Transcription (Speech-to-Text) Module
//!
//! This module provides transcription capabilities using various STT providers.
//!
//! # Supported Providers
//!
//! - **Groq** - Whisper Large V3 Turbo (fast, generous free tier)
//! - **OpenAI** - Whisper API
//!
//! # Examples
//!
//! ```rust,ignore
//! use mofa_foundation::llm::transcription::{TranscriptionProvider, GroqTranscriptionProvider};
//!
//! // Create Groq transcription provider
//! let provider = GroqTranscriptionProvider::new("your-groq-api-key");
//!
//! // Transcribe an audio file
//! let transcript = provider.transcribe("/path/to/audio.ogg").await?;
//! println!("Transcript: {}", transcript);
//! ```

use mofa_kernel::agent::types::error::{GlobalError, GlobalResult};
use async_trait::async_trait;
use reqwest::Client;
use serde::Deserialize;
use std::path::Path;
use std::time::Duration;
use tracing::{debug, warn};

/// Trait for audio transcription providers
#[async_trait]
pub trait TranscriptionProvider: Send + Sync {
    /// Transcribe an audio file
    ///
    /// # Arguments
    /// - `file_path`: Path to the audio file
    ///
    /// # Returns
    /// The transcribed text
    async fn transcribe(&self, file_path: &Path) -> GlobalResult<String>;

    /// Check if the provider is configured with an API key
    fn is_configured(&self) -> bool;
}

/// Groq transcription provider using Whisper Large V3
///
/// Groq offers extremely fast transcription with a generous free tier.
///
/// # Example
///
/// ```rust,ignore
/// use mofa_foundation::llm::transcription::GroqTranscriptionProvider;
///
/// let provider = GroqTranscriptionProvider::new("your-api-key");
/// let transcript = provider.transcribe("audio.ogg").await?;
/// ```
pub struct GroqTranscriptionProvider {
    client: Client,
    api_key: String,
    api_url: String,
}

impl GroqTranscriptionProvider {
    /// Create a new Groq transcription provider
    pub fn new(api_key: impl Into<String>) -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(120))
            .build()
            .expect("Failed to create HTTP client");

        Self {
            client,
            api_key: api_key.into(),
            api_url: "https://api.groq.com/openai/v1/audio/transcriptions".to_string(),
        }
    }

    /// Create from environment variable `GROQ_API_KEY`
    pub fn from_env() -> Option<Self> {
        let api_key = std::env::var("GROQ_API_KEY").ok()?;
        if api_key.is_empty() {
            None
        } else {
            Some(Self::new(api_key))
        }
    }

    /// Set a custom API URL (for testing or custom endpoints)
    pub fn with_api_url(mut self, url: impl Into<String>) -> Self {
        self.api_url = url.into();
        self
    }
}

impl Default for GroqTranscriptionProvider {
    fn default() -> Self {
        Self::new("")
    }
}

#[async_trait]
impl TranscriptionProvider for GroqTranscriptionProvider {
    async fn transcribe(&self, file_path: &Path) -> GlobalResult<String> {
        if !self.is_configured() {
            return Ok(String::new());
        }

        if !file_path.exists() {
            return Err(GlobalError::Other(format!(
                "Audio file not found: {}",
                file_path.display()
            )));
        }

        debug!("Transcribing audio file: {}", file_path.display());

        // Read the file content
        let file_content = tokio::fs::read(file_path)
            .await
            .map_err(|e| GlobalError::Other(format!("Failed to read audio file: {}", e)))?;

        // Get the file name
        let file_name = file_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("audio.ogg");

        // Create multipart form
        let part = reqwest::multipart::Part::bytes(file_content)
            .file_name(file_name.to_string())
            .mime_str("audio/*")
            .map_err(|e| GlobalError::Other(format!("Failed to create multipart: {}", e)))?;

        let form = reqwest::multipart::Form::new()
            .part("file", part)
            .text("model", "whisper-large-v3-turbo");

        // Send request
        let response = self
            .client
            .post(&self.api_url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .multipart(form)
            .send()
            .await
            .map_err(|e| GlobalError::Other(format!("Groq API error: {}", e)))?;

        let status = response.status();
        let response_text = response
            .text()
            .await
            .map_err(|e| GlobalError::Other(format!("Failed to read response: {}", e)))?;

        if !status.is_success() {
            warn!("Groq API error: {}", response_text);
            return Err(GlobalError::Other(format!(
                "Groq API returned status {}: {}",
                status,
                response_text
            )));
        }

        // Parse response
        #[derive(Deserialize)]
        struct TranscriptionResponse {
            text: String,
        }

        if let Ok(data) = serde_json::from_str::<TranscriptionResponse>(&response_text) {
            debug!("Transcription successful: {} chars", data.text.len());
            Ok(data.text)
        } else {
            warn!("Failed to parse transcription response: {}", response_text);
            Ok(String::new())
        }
    }

    fn is_configured(&self) -> bool {
        !self.api_key.is_empty()
    }
}

// ============================================================================
// OpenAI Whisper Provider
// ============================================================================

/// OpenAI transcription provider using Whisper API
///
/// # Example
///
/// ```rust,ignore
/// use mofa_foundation::llm::transcription::OpenAITranscriptionProvider;
///
/// let provider = OpenAITranscriptionProvider::new("your-openai-api-key");
/// let transcript = provider.transcribe("audio.mp3").await?;
/// ```
pub struct OpenAITranscriptionProvider {
    client: Client,
    api_key: String,
    api_url: String,
}

impl OpenAITranscriptionProvider {
    /// Create a new OpenAI transcription provider
    pub fn new(api_key: impl Into<String>) -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(120))
            .build()
            .expect("Failed to create HTTP client");

        Self {
            client,
            api_key: api_key.into(),
            api_url: "https://api.openai.com/v1/audio/transcriptions".to_string(),
        }
    }

    /// Create from environment variable `OPENAI_API_KEY`
    pub fn from_env() -> Option<Self> {
        let api_key = std::env::var("OPENAI_API_KEY").ok()?;
        if api_key.is_empty() {
            None
        } else {
            Some(Self::new(api_key))
        }
    }

    /// Set a custom API URL (for Azure OpenAI or compatible endpoints)
    pub fn with_api_url(mut self, url: impl Into<String>) -> Self {
        self.api_url = url.into();
        self
    }
}

impl Default for OpenAITranscriptionProvider {
    fn default() -> Self {
        Self::new("")
    }
}

#[async_trait]
impl TranscriptionProvider for OpenAITranscriptionProvider {
    async fn transcribe(&self, file_path: &Path) -> GlobalResult<String> {
        if !self.is_configured() {
            return Ok(String::new());
        }

        if !file_path.exists() {
            return Err(GlobalError::Other(format!(
                "Audio file not found: {}",
                file_path.display()
            )));
        }

        debug!(
            "Transcribing audio file with OpenAI: {}",
            file_path.display()
        );

        // Read the file content
        let file_content = tokio::fs::read(file_path)
            .await
            .map_err(|e| GlobalError::Other(format!("Failed to read audio file: {}", e)))?;

        // Get the file name
        let file_name = file_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("audio.mp3");

        // Create multipart form
        let part = reqwest::multipart::Part::bytes(file_content)
            .file_name(file_name.to_string())
            .mime_str("audio/mp3")
            .map_err(|e| GlobalError::Other(format!("Failed to create multipart: {}", e)))?;

        let form = reqwest::multipart::Form::new()
            .part("file", part)
            .text("model", "whisper-1");

        // Send request
        let response = self
            .client
            .post(&self.api_url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .multipart(form)
            .send()
            .await
            .map_err(|e| GlobalError::Other(format!("OpenAI API error: {}", e)))?;

        let status = response.status();
        let response_text = response
            .text()
            .await
            .map_err(|e| GlobalError::Other(format!("Failed to read response: {}", e)))?;

        if !status.is_success() {
            warn!("OpenAI API error: {}", response_text);
            return Err(GlobalError::Other(format!(
                "OpenAI API returned status {}: {}",
                status,
                response_text
            )));
        }

        // Parse response
        #[derive(Deserialize)]
        struct TranscriptionResponse {
            text: String,
        }

        if let Ok(data) = serde_json::from_str::<TranscriptionResponse>(&response_text) {
            debug!("Transcription successful: {} chars", data.text.len());
            Ok(data.text)
        } else {
            warn!("Failed to parse transcription response: {}", response_text);
            Ok(String::new())
        }
    }

    fn is_configured(&self) -> bool {
        !self.api_key.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_groq_provider_creation() {
        let provider = GroqTranscriptionProvider::new("test-key");
        assert_eq!(provider.api_key, "test-key");
        assert!(provider.is_configured());
    }

    #[test]
    fn test_groq_provider_empty_key() {
        let provider = GroqTranscriptionProvider::new("");
        assert!(!provider.is_configured());
    }

    #[test]
    fn test_groq_provider_default() {
        let provider = GroqTranscriptionProvider::default();
        assert!(!provider.is_configured());
    }

    #[test]
    fn test_openai_provider_creation() {
        let provider = OpenAITranscriptionProvider::new("test-key");
        assert_eq!(provider.api_key, "test-key");
        assert!(provider.is_configured());
    }

    #[test]
    fn test_openai_provider_custom_url() {
        let provider = OpenAITranscriptionProvider::new("test-key")
            .with_api_url("https://custom.api/v1/audio/transcriptions");
        assert_eq!(
            provider.api_url,
            "https://custom.api/v1/audio/transcriptions"
        );
    }
}
