//! Anthropic Claude Provider
//!
//! Lightweight implementation of Anthropic's Messages API (Claude 3+).
//! Focused on text chat; tooling/vision can be added later.

use super::provider::{ChatStream, LLMProvider, ModelCapabilities, ModelInfo};
use super::types::*;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::time::Duration;

/// Anthropic provider configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnthropicConfig {
    /// API key
    pub api_key: String,
    /// Base URL, e.g. https://api.anthropic.com
    pub base_url: String,
    /// API version header value
    pub version: String,
    /// Default model
    pub default_model: String,
    /// Default max output tokens (required by Anthropic)
    pub default_max_tokens: u32,
    /// Default temperature
    pub default_temperature: f32,
    /// Request timeout (seconds)
    pub timeout_secs: u64,
}

impl Default for AnthropicConfig {
    fn default() -> Self {
        Self {
            api_key: String::new(),
            base_url: "https://api.anthropic.com".to_string(),
            // As of 2026-02 Anthropic docs still reference 2023-06-01 version
            version: "2023-06-01".to_string(),
            default_model: "claude-3.5-sonnet-20241022".to_string(),
            default_max_tokens: 4096,
            default_temperature: 0.7,
            timeout_secs: 60,
        }
    }
}

impl AnthropicConfig {
    /// Create config from API key
    pub fn new(api_key: impl Into<String>) -> Self {
        Self {
            api_key: api_key.into(),
            ..Default::default()
        }
    }

    /// Build from environment variables
    pub fn from_env() -> Self {
        let mut cfg = Self {
            api_key: std::env::var("ANTHROPIC_API_KEY").unwrap_or_default(),
            ..Default::default()
        };

        if let Ok(model) = std::env::var("ANTHROPIC_MODEL") {
            cfg.default_model = model;
        }
        if let Ok(base_url) = std::env::var("ANTHROPIC_BASE_URL") {
            cfg.base_url = base_url;
        }
        if let Ok(version) = std::env::var("ANTHROPIC_VERSION") {
            cfg.version = version;
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

    pub fn with_version(mut self, version: impl Into<String>) -> Self {
        self.version = version.into();
        self
    }

    pub fn with_timeout(mut self, secs: u64) -> Self {
        self.timeout_secs = secs;
        self
    }
}

/// Anthropic Claude provider
pub struct AnthropicProvider {
    client: reqwest::Client,
    config: AnthropicConfig,
}

impl AnthropicProvider {
    pub fn new(api_key: impl Into<String>) -> Self {
        Self::with_config(AnthropicConfig::new(api_key))
    }

    pub fn from_env() -> Self {
        Self::with_config(AnthropicConfig::from_env())
    }

    pub fn with_config(config: AnthropicConfig) -> Self {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(config.timeout_secs))
            .build()
            .expect("Failed to build reqwest client");

        Self { client, config }
    }

    fn convert_messages(messages: &[ChatMessage]) -> (Option<String>, Vec<serde_json::Value>) {
        let mut system_parts = Vec::new();
        let mut converted = Vec::new();

        for msg in messages {
            match msg.role {
                Role::System => {
                    if let Some(text) = msg.text_content() {
                        system_parts.push(text.to_string());
                    }
                }
                Role::User | Role::Assistant | Role::Tool => {
                    let role = match msg.role {
                        Role::User => "user",
                        Role::Assistant => "assistant",
                        Role::Tool => "user", // tool results are surfaced to user role
                        Role::System => unreachable!(),
                    };

                    let text = match &msg.content {
                        Some(MessageContent::Text(t)) => t.clone(),
                        Some(MessageContent::Parts(parts)) => parts
                            .iter()
                            .filter_map(|p| match p {
                                ContentPart::Text { text } => Some(text.clone()),
                                _ => None,
                            })
                            .collect::<Vec<_>>()
                            .join("\n"),
                        None => String::new(),
                    };

                    converted.push(serde_json::json!({
                        "role": role,
                        "content": [{"type": "text", "text": text}],
                    }));
                }
            }
        }

        let system = if system_parts.is_empty() {
            None
        } else {
            Some(system_parts.join("\n"))
        };

        (system, converted)
    }

    fn map_error(err: reqwest::Error) -> LLMError {
        if err.is_timeout() {
            LLMError::Timeout(err.to_string())
        } else if err.is_connect() || err.is_request() {
            LLMError::NetworkError(err.to_string())
        } else {
            LLMError::Other(err.to_string())
        }
    }
}

#[derive(Debug, Deserialize)]
struct AnthropicUsage {
    #[serde(rename = "input_tokens")]
    input_tokens: u32,
    #[serde(rename = "output_tokens")]
    output_tokens: u32,
}

#[derive(Debug, Deserialize)]
struct AnthropicContentBlock {
    #[serde(rename = "type")]
    kind: String,
    text: Option<String>,
}

#[derive(Debug, Deserialize)]
struct AnthropicMessageResponse {
    id: String,
    model: String,
    content: Vec<AnthropicContentBlock>,
    #[serde(rename = "stop_reason")]
    stop_reason: Option<String>,
    #[serde(rename = "stop_sequence")]
    stop_sequence: Option<String>,
    usage: Option<AnthropicUsage>,
}

#[async_trait]
impl LLMProvider for AnthropicProvider {
    fn name(&self) -> &str {
        "anthropic"
    }

    fn default_model(&self) -> &str {
        &self.config.default_model
    }

    fn supports_streaming(&self) -> bool {
        false
    }

    fn supports_tools(&self) -> bool {
        false
    }

    fn supports_vision(&self) -> bool {
        false
    }

    async fn chat(&self, request: ChatCompletionRequest) -> LLMResult<ChatCompletionResponse> {
        let (system_prompt, messages) = Self::convert_messages(&request.messages);

        let model = if request.model.is_empty() {
            self.config.default_model.clone()
        } else {
            request.model.clone()
        };

        let max_tokens = request.max_tokens.unwrap_or(self.config.default_max_tokens);

        let mut body = serde_json::json!({
            "model": model,
            "messages": messages,
            "max_tokens": max_tokens,
        });

        let temperature = request
            .temperature
            .unwrap_or(self.config.default_temperature);
        body["temperature"] = serde_json::json!(temperature);

        if let Some(tp) = request.top_p {
            body["top_p"] = serde_json::json!(tp);
        }

        if let Some(stop) = request.stop.clone() {
            body["stop_sequences"] = serde_json::json!(stop);
        }

        if let Some(sys) = system_prompt {
            body["system"] = serde_json::json!(sys);
        }

        let url = format!("{}/v1/messages", self.config.base_url.trim_end_matches('/'));

        let resp = self
            .client
            .post(&url)
            .header("x-api-key", &self.config.api_key)
            .header("anthropic-version", &self.config.version)
            .json(&body)
            .send()
            .await
            .map_err(Self::map_error)?;

        let status = resp.status();
        let text = resp.text().await.map_err(Self::map_error)?;

        if !status.is_success() {
            return Err(LLMError::ApiError {
                code: Some(status.as_u16().to_string()),
                message: text,
            });
        }

        let parsed: AnthropicMessageResponse =
            serde_json::from_str(&text).map_err(|e| LLMError::SerializationError(e.to_string()))?;

        let content_text = parsed
            .content
            .iter()
            .filter_map(|c| c.text.clone())
            .collect::<Vec<_>>()
            .join("");

        let finish_reason = match parsed.stop_reason.as_deref() {
            Some("end_turn") | Some("stop_sequence") | Some("stop") => Some(FinishReason::Stop),
            Some("max_tokens") => Some(FinishReason::Length),
            _ => None,
        };

        let usage = parsed.usage.map(|u| Usage {
            prompt_tokens: u.input_tokens,
            completion_tokens: u.output_tokens,
            total_tokens: u.input_tokens + u.output_tokens,
        });

        let choice = Choice {
            index: 0,
            message: ChatMessage {
                role: Role::Assistant,
                content: Some(MessageContent::Text(content_text)),
                name: None,
                tool_calls: None,
                tool_call_id: None,
            },
            finish_reason,
            logprobs: None,
        };

        let created = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_else(|_| Duration::from_secs(0))
            .as_secs();

        Ok(ChatCompletionResponse {
            id: parsed.id,
            object: "chat.completion".to_string(),
            created,
            model: parsed.model,
            choices: vec![choice],
            usage,
            system_fingerprint: None,
        })
    }

    async fn chat_stream(&self, _request: ChatCompletionRequest) -> LLMResult<ChatStream> {
        Err(LLMError::ProviderNotSupported(
            "Anthropic streaming not yet implemented".to_string(),
        ))
    }

    async fn health_check(&self) -> LLMResult<bool> {
        let request = ChatCompletionRequest::new(self.default_model())
            .system("Say 'ok'")
            .max_tokens(5);

        self.chat(request).await.map(|_| true).or(Ok(false))
    }

    async fn get_model_info(&self, model: &str) -> LLMResult<ModelInfo> {
        Ok(ModelInfo {
            id: model.to_string(),
            name: model.to_string(),
            description: None,
            context_window: None,
            max_output_tokens: Some(self.config.default_max_tokens),
            training_cutoff: None,
            capabilities: ModelCapabilities {
                streaming: false,
                tools: false,
                vision: false,
                json_mode: true,
                json_schema: false,
            },
        })
    }
}
