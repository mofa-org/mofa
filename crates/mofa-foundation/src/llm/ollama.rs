//! Ollama Provider (thin wrapper of OpenAI)
//!

use super::openai::{OpenAIConfig, OpenAIProvider};
use super::provider::{ChatStream, LLMProvider, ModelCapabilities, ModelInfo};
use super::types::*;
use async_trait::async_trait;

/// Ollama provider configuration
#[derive(Debug, Clone)]
pub struct OllamaConfig {
    /// API key not needed

    /// Base URL (default: http://localhost:11434/v1)
    pub base_url: String,
    /// Default model id, e.g., llama3
    pub default_model: String,
    /// Default temperature
    pub default_temperature: f32,
    /// Default max output tokens
    pub default_max_tokens: u32,
    /// Request timeout
    pub timeout_secs: u64,
}

impl Default for OllamaConfig {
    fn default() -> Self {
        Self {
            base_url: "http://localhost:11434/v1".to_string(),
            default_model: "llama3".to_string(),
            default_temperature: 0.7,
            default_max_tokens: 2048,
            timeout_secs: 60,
        }
    }
}

impl OllamaConfig {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn from_env() -> Self {
        let mut cfg = Self::default();

        if let Ok(model) = std::env::var("OLLAMA_MODEL") {
            cfg.default_model = model;
        }
        if let Ok(base_url) = std::env::var("OLLAMA_BASE_URL") {
            let base = base_url.trim_end_matches('/');
            cfg.base_url = if base.ends_with("/v1") {
                base.to_string()
            } else {
                format!("{}/v1", base)
            } //solve any ambiguity on the url
        }
        cfg
    }

    pub fn with_base_url(mut self, url: impl Into<String>) -> Self {
        self.base_url = url.into();
        self
    }

    pub fn with_model(mut self, model: impl Into<String>) -> Self {
        self.default_model = model.into();
        self
    }

    pub fn with_temperature(mut self, temp: f32) -> Self {
        self.default_temperature = temp;
        self
    }

    pub fn with_max_tokens(mut self, tokens: u32) -> Self {
        self.default_max_tokens = tokens;
        self
    }

    pub fn with_timeout(mut self, secs: u64) -> Self {
        self.timeout_secs = secs;
        self
    }
}

/// Ollama provider (just like OpenAIProvider --> inner)
pub struct OllamaProvider {
    inner: OpenAIProvider,
}

impl Default for OllamaProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl OllamaProvider {
    /// Create a provider with the default localhost endpoint and given model.
    pub fn new() -> Self {
        let config = OllamaConfig::new();
        Self::with_config(config)
    }

    /// Create a provider reading `OLLAMA_BASE_URL` and `OLLAMA_MODEL` from the environment.
    pub fn from_env() -> Self {
        Self::with_config(OllamaConfig::from_env())
    }

    /// Create a provider from an explicit `OllamaConfig`.
    pub fn with_config(config: OllamaConfig) -> Self {
        let openai_config = OpenAIConfig::new("not-needed")
            .with_base_url(&config.base_url)
            .with_model(&config.default_model)
            .with_temperature(config.default_temperature)
            .with_max_tokens(config.default_max_tokens)
            .with_timeout(config.timeout_secs);
        Self {
            inner: OpenAIProvider::with_config(openai_config),
        }
    }
}

#[async_trait]
impl LLMProvider for OllamaProvider {
    fn name(&self) -> &str {
        "ollama"
    }

    fn default_model(&self) -> &str {
        self.inner.default_model()
    }

    fn supports_streaming(&self) -> bool {
        self.inner.supports_streaming()
    }

    fn supports_tools(&self) -> bool {
        self.inner.supports_tools()
    }

    fn supports_vision(&self) -> bool {
        self.inner.supports_vision()
    }

    async fn chat(&self, request: ChatCompletionRequest) -> LLMResult<ChatCompletionResponse> {
        self.inner.chat(request).await
    }

    async fn chat_stream(&self, request: ChatCompletionRequest) -> LLMResult<ChatStream> {
        self.inner.chat_stream(request).await
    }

    async fn embedding(&self, request: EmbeddingRequest) -> LLMResult<EmbeddingResponse> {
        self.inner.embedding(request).await
    }

    async fn health_check(&self) -> LLMResult<bool> {
        self.inner.health_check().await
    }

    async fn get_model_info(&self, model: &str) -> LLMResult<ModelInfo> {
        let info = match model {
            "llama3" | "llama3:latest" | "llama3:8b" => ModelInfo {
                id: model.to_string(),
                name: "Llama 3 8B".to_string(),
                description: Some("Meta Llama 3 8B instruction-tuned model".to_string()),
                context_window: Some(8192),
                max_output_tokens: Some(2048),
                training_cutoff: Some("2023-12".to_string()),
                capabilities: ModelCapabilities {
                    streaming: true,
                    tools: true,
                    vision: false,
                    json_mode: true,
                    json_schema: false,
                },
            },
            "llama3:70b" => ModelInfo {
                id: model.to_string(),
                name: "Llama 3 70B".to_string(),
                description: Some("Meta Llama 3 70B instruction-tuned model".to_string()),
                context_window: Some(8192),
                max_output_tokens: Some(2048),
                training_cutoff: Some("2023-12".to_string()),
                capabilities: ModelCapabilities {
                    streaming: true,
                    tools: true,
                    vision: false,
                    json_mode: true,
                    json_schema: false,
                },
            },
            "llama3.1" | "llama3.1:latest" | "llama3.1:8b" => ModelInfo {
                id: model.to_string(),
                name: "Llama 3.1 8B".to_string(),
                description: Some("Meta Llama 3.1 8B with 128k context window".to_string()),
                context_window: Some(131072),
                max_output_tokens: Some(4096),
                training_cutoff: Some("2024-03".to_string()),
                capabilities: ModelCapabilities {
                    streaming: true,
                    tools: true,
                    vision: false,
                    json_mode: true,
                    json_schema: false,
                },
            },
            "llama3.1:70b" => ModelInfo {
                id: model.to_string(),
                name: "Llama 3.1 70B".to_string(),
                description: Some("Meta Llama 3.1 70B with 128k context window".to_string()),
                context_window: Some(131072),
                max_output_tokens: Some(4096),
                training_cutoff: Some("2024-03".to_string()),
                capabilities: ModelCapabilities {
                    streaming: true,
                    tools: true,
                    vision: false,
                    json_mode: true,
                    json_schema: false,
                },
            },
            "llama3.2" | "llama3.2:latest" | "llama3.2:3b" => ModelInfo {
                id: model.to_string(),
                name: "Llama 3.2 3B".to_string(),
                description: Some("Meta Llama 3.2 3B lightweight model".to_string()),
                context_window: Some(131072),
                max_output_tokens: Some(4096),
                training_cutoff: Some("2024-06".to_string()),
                capabilities: ModelCapabilities {
                    streaming: true,
                    tools: false,
                    vision: false,
                    json_mode: true,
                    json_schema: false,
                },
            },
            "llama2" | "llama2:latest" | "llama2:7b" => ModelInfo {
                id: model.to_string(),
                name: "Llama 2 7B".to_string(),
                description: Some("Meta Llama 2 7B chat model".to_string()),
                context_window: Some(4096),
                max_output_tokens: Some(2048),
                training_cutoff: Some("2023-07".to_string()),
                capabilities: ModelCapabilities {
                    streaming: true,
                    tools: false,
                    vision: false,
                    json_mode: false,
                    json_schema: false,
                },
            },
            "mistral" | "mistral:latest" | "mistral:7b" => ModelInfo {
                id: model.to_string(),
                name: "Mistral 7B".to_string(),
                description: Some("Mistral AI 7B instruction-tuned model".to_string()),
                context_window: Some(32768),
                max_output_tokens: Some(4096),
                training_cutoff: None,
                capabilities: ModelCapabilities {
                    streaming: true,
                    tools: true,
                    vision: false,
                    json_mode: true,
                    json_schema: false,
                },
            },
            _ => ModelInfo {
                id: model.to_string(),
                name: model.to_string(),
                description: None,
                context_window: None,
                max_output_tokens: None,
                training_cutoff: None,
                capabilities: ModelCapabilities::default(),
            },
        };

        Ok(info)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_defaults() {
        let config = OllamaConfig::default();
        assert_eq!(config.base_url, "http://localhost:11434/v1");
        assert_eq!(config.default_model, "llama3");
    }

    #[test]
    fn test_config_builder() {
        let config = OllamaConfig::new()
            .with_base_url("http://localhost:11434/v1")
            .with_model("mistral")
            .with_temperature(0.5)
            .with_max_tokens(1024);

        assert_eq!(config.base_url, "http://localhost:11434/v1");
        assert_eq!(config.default_model, "mistral");
        assert_eq!(config.default_temperature, 0.5);
        assert_eq!(config.default_max_tokens, 1024);
    }

    #[test]
    fn test_provider_name() {
        let config = OllamaConfig::new()
            .with_model("llama3")
            .with_base_url("http://localhost:11434/v1");

        let provider = OllamaProvider::with_config(config);
        assert_eq!(provider.name(), "ollama");
    }
}
