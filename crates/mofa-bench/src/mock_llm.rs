//! Mock LLM Backend
//!
//! A lightweight, deterministic mock LLM for benchmark use.
//! Returns configurable responses with optional artificial latency.
//! Zero external API calls — no cost, no rate limits, fully reproducible.

use std::time::Duration;

/// Configuration for the mock LLM backend.
#[derive(Debug, Clone)]
pub struct MockLlmConfig {
    /// Fixed response text returned by the mock.
    pub response_text: String,
    /// Artificial latency to simulate real LLM response time.
    pub latency: Option<Duration>,
    /// Number of tokens to report in usage stats.
    pub prompt_tokens: u32,
    /// Number of completion tokens to report.
    pub completion_tokens: u32,
    /// Whether to simulate streaming mode.
    pub streaming: bool,
    /// Chunk size for streaming (characters per chunk).
    pub stream_chunk_size: usize,
}

impl Default for MockLlmConfig {
    fn default() -> Self {
        Self {
            response_text: "This is a mock LLM response for benchmarking purposes.".into(),
            latency: None,
            prompt_tokens: 50,
            completion_tokens: 25,
            streaming: false,
            stream_chunk_size: 10,
        }
    }
}

impl MockLlmConfig {
    /// Create a config optimized for benchmarking (no latency).
    pub fn for_bench() -> Self {
        Self::default()
    }

    /// Create a config with a small response.
    pub fn small() -> Self {
        Self {
            response_text: "OK".into(),
            prompt_tokens: 5,
            completion_tokens: 1,
            ..Self::default()
        }
    }

    /// Create a config with a large response (~10KB).
    pub fn large() -> Self {
        let response = "The quick brown fox jumps over the lazy dog. ".repeat(250);
        Self {
            response_text: response,
            prompt_tokens: 500,
            completion_tokens: 2500,
            ..Self::default()
        }
    }

    /// Create a config that simulates streaming.
    pub fn streaming() -> Self {
        Self {
            streaming: true,
            stream_chunk_size: 10,
            ..Self::default()
        }
    }
}

/// A deterministic mock LLM backend for benchmarking.
#[derive(Debug, Clone)]
pub struct MockLlmBackend {
    config: MockLlmConfig,
}

impl MockLlmBackend {
    /// Create a new mock LLM backend with the given config.
    pub fn new(config: MockLlmConfig) -> Self {
        Self { config }
    }

    /// Simulate a synchronous (non-streaming) LLM call.
    /// Returns the configured response text.
    pub fn generate(&self, _prompt: &str) -> MockLlmResponse {
        MockLlmResponse {
            text: self.config.response_text.clone(),
            prompt_tokens: self.config.prompt_tokens,
            completion_tokens: self.config.completion_tokens,
        }
    }

    /// Simulate a synchronous call with optional latency.
    pub async fn generate_async(&self, prompt: &str) -> MockLlmResponse {
        if let Some(latency) = self.config.latency {
            tokio::time::sleep(latency).await;
        }
        self.generate(prompt)
    }

    /// Simulate streaming by returning chunks of the response.
    pub fn generate_stream(&self, _prompt: &str) -> Vec<String> {
        let text = &self.config.response_text;
        let chunk_size = self.config.stream_chunk_size.max(1);
        text.chars()
            .collect::<Vec<_>>()
            .chunks(chunk_size)
            .map(|chunk| chunk.iter().collect())
            .collect()
    }

    /// Get the underlying config.
    pub fn config(&self) -> &MockLlmConfig {
        &self.config
    }
}

/// Response from the mock LLM.
#[derive(Debug, Clone)]
pub struct MockLlmResponse {
    /// The generated text.
    pub text: String,
    /// Number of prompt tokens consumed.
    pub prompt_tokens: u32,
    /// Number of completion tokens generated.
    pub completion_tokens: u32,
}

impl MockLlmResponse {
    /// Total tokens (prompt + completion).
    pub fn total_tokens(&self) -> u32 {
        self.prompt_tokens + self.completion_tokens
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mock_generate() {
        let backend = MockLlmBackend::new(MockLlmConfig::for_bench());
        let response = backend.generate("Hello");
        assert!(!response.text.is_empty());
        assert_eq!(response.prompt_tokens, 50);
        assert_eq!(response.completion_tokens, 25);
        assert_eq!(response.total_tokens(), 75);
    }

    #[test]
    fn test_mock_streaming() {
        let backend = MockLlmBackend::new(MockLlmConfig::streaming());
        let chunks = backend.generate_stream("Hello");
        assert!(chunks.len() > 1);
        let reassembled: String = chunks.into_iter().collect();
        assert_eq!(reassembled, MockLlmConfig::default().response_text);
    }

    #[test]
    fn test_mock_large() {
        let backend = MockLlmBackend::new(MockLlmConfig::large());
        let response = backend.generate("Hello");
        assert!(response.text.len() > 10_000);
    }
}
