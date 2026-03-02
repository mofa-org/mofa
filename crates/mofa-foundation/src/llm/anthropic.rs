//! Anthropic Claude Provider
//!
//! Implementation of Anthropic's Messages API (Claude 3+) with streaming support.
//! Parses Anthropic SSE events (`message_start`, `content_block_delta`, `message_delta`,
//! `message_stop`) and maps them to `ChatCompletionChunk`.

use super::provider::{ChatStream, LLMProvider, ModelCapabilities, ModelInfo};
use super::types::*;
use async_trait::async_trait;
use futures::StreamExt;
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

                    let mut contents = Vec::new();
                    match &msg.content {
                        Some(MessageContent::Text(t)) => {
                            contents.push(serde_json::json!({
                                "type": "text",
                                "text": t.clone(),
                            }));
                        }
                        Some(MessageContent::Parts(parts)) => {
                            for part in parts {
                                match part {
                                    ContentPart::Text { text } => {
                                        contents.push(serde_json::json!({
                                            "type": "text",
                                            "text": text.clone(),
                                        }));
                                    }
                                    ContentPart::Image { image_url } => {
                                        let media_type = if image_url.url.contains("data:image/jpeg") {
                                            "image/jpeg"
                                        } else if image_url.url.contains("data:image/png") {
                                            "image/png"
                                        } else if image_url.url.contains("data:image/webp") {
                                            "image/webp"
                                        } else {
                                            "image/jpeg" // Default
                                        };
                                        let data = image_url.url.split(',').last().unwrap_or(&image_url.url);
                                        contents.push(serde_json::json!({
                                            "type": "image",
                                            "source": {
                                                "type": "base64",
                                                "media_type": media_type,
                                                "data": data,
                                            }
                                        }));
                                    }
                                    ContentPart::Audio { audio } => {
                                        let media_type = format!("audio/{}", audio.format.to_lowercase());
                                        let data = audio.data.split(',').last().unwrap_or(&audio.data);
                                        // Some providers/models may not support this block, but this is the standard Anthropics structure if/when supported.
                                        contents.push(serde_json::json!({
                                            "type": "audio",
                                            "source": {
                                                "type": "base64",
                                                "media_type": media_type,
                                                "data": data,
                                            }
                                        }));
                                    }
                                    ContentPart::Video { video } => {
                                        let media_type = format!("video/{}", video.format.to_lowercase());
                                        let data = video.data.split(',').last().unwrap_or(&video.data);
                                        contents.push(serde_json::json!({
                                            "type": "video",
                                            "source": {
                                                "type": "base64",
                                                "media_type": media_type,
                                                "data": data,
                                            }
                                        }));
                                    }
                                }
                            }
                        }
                        None => {}
                    }

                    converted.push(serde_json::json!({
                        "role": role,
                        "content": contents,
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

    /// Build the JSON request body (shared between chat and chat_stream)
    fn build_request_body(&self, request: &ChatCompletionRequest, stream: bool) -> serde_json::Value {
        let (system_prompt, messages) = Self::convert_messages(&request.messages);

        let model = if request.model.is_empty() {
            self.config.default_model.clone()
        } else {
            request.model.clone()
        };

        let max_tokens = request.max_tokens.unwrap_or(self.config.default_max_tokens);
        let temperature = request
            .temperature
            .unwrap_or(self.config.default_temperature);

        let mut body = serde_json::json!({
            "model": model,
            "messages": messages,
            "max_tokens": max_tokens,
            "temperature": temperature,
        });

        if stream {
            body["stream"] = serde_json::json!(true);
        }

        if let Some(tp) = request.top_p {
            body["top_p"] = serde_json::json!(tp);
        }

        if let Some(ref stop) = request.stop {
            body["stop_sequences"] = serde_json::json!(stop);
        }

        if let Some(sys) = system_prompt {
            body["system"] = serde_json::json!(sys);
        }

        body
    }
}

#[derive(Debug, Deserialize)]
struct AnthropicUsage {
    #[serde(rename = "input_tokens", default)]
    input_tokens: u32,
    #[serde(rename = "output_tokens", default)]
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
    #[allow(dead_code)]
    stop_sequence: Option<String>,
    usage: Option<AnthropicUsage>,
}

/// `message_start` event data.
#[derive(Debug, Deserialize)]
struct AnthropicMessageStart {
    message: AnthropicMessageStartInner,
}

#[derive(Debug, Deserialize)]
struct AnthropicMessageStartInner {
    id: String,
    model: String,
    usage: Option<AnthropicUsage>,
}

/// `content_block_delta` event data.
#[derive(Debug, Deserialize)]
struct AnthropicContentBlockDelta {
    delta: AnthropicTextDelta,
}

#[derive(Debug, Deserialize)]
struct AnthropicTextDelta {
    #[serde(default)]
    text: String,
}

/// `message_delta` event data.
#[derive(Debug, Deserialize)]
struct AnthropicMessageDelta {
    delta: AnthropicMessageDeltaInner,
    usage: Option<AnthropicUsage>,
}

#[derive(Debug, Deserialize)]
struct AnthropicMessageDeltaInner {
    stop_reason: Option<String>,
}

/// Result of processing a single SSE data line
enum SseAction {
    Emit(LLMResult<ChatCompletionChunk>),
    Stop,
}

/// Process a single SSE `data:` payload given the current `event_type`
fn parse_sse_event(
    event_type: &str,
    json_str: &str,
    msg_id: &mut String,
    model: &mut String,
) -> Option<SseAction> {
    let now = || {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs()
    };

    match event_type {
        "message_start" => {
            let ms: AnthropicMessageStart = serde_json::from_str(json_str).ok()?;
            *msg_id = ms.message.id.clone();
            *model = ms.message.model.clone();
            let usage = ms.message.usage.map(|u| Usage {
                prompt_tokens: u.input_tokens,
                completion_tokens: u.output_tokens,
                total_tokens: u.input_tokens + u.output_tokens,
            });
            Some(SseAction::Emit(Ok(ChatCompletionChunk {
                id: msg_id.clone(),
                object: "chat.completion.chunk".to_string(),
                created: now(),
                model: model.clone(),
                choices: vec![ChunkChoice {
                    index: 0,
                    delta: ChunkDelta {
                        role: Some(Role::Assistant),
                        content: None,
                        tool_calls: None,
                    },
                    finish_reason: None,
                }],
                usage,
            })))
        }
        "content_block_delta" => {
            let cbd: AnthropicContentBlockDelta = serde_json::from_str(json_str).ok()?;
            Some(SseAction::Emit(Ok(ChatCompletionChunk {
                id: msg_id.clone(),
                object: "chat.completion.chunk".to_string(),
                created: now(),
                model: model.clone(),
                choices: vec![ChunkChoice {
                    index: 0,
                    delta: ChunkDelta {
                        role: None,
                        content: Some(cbd.delta.text),
                        tool_calls: None,
                    },
                    finish_reason: None,
                }],
                usage: None,
            })))
        }
        "message_delta" => {
            let md: AnthropicMessageDelta = serde_json::from_str(json_str).ok()?;
            let finish_reason = match md.delta.stop_reason.as_deref() {
                Some("end_turn") | Some("stop_sequence") | Some("stop") => Some(FinishReason::Stop),
                Some("max_tokens") => Some(FinishReason::Length),
                _ => None,
            };
            let usage = md.usage.map(|u| Usage {
                prompt_tokens: u.input_tokens,
                completion_tokens: u.output_tokens,
                total_tokens: u.input_tokens + u.output_tokens,
            });
            Some(SseAction::Emit(Ok(ChatCompletionChunk {
                id: msg_id.clone(),
                object: "chat.completion.chunk".to_string(),
                created: now(),
                model: model.clone(),
                choices: vec![ChunkChoice {
                    index: 0,
                    delta: ChunkDelta {
                        role: None,
                        content: None,
                        tool_calls: None,
                    },
                    finish_reason,
                }],
                usage,
            })))
        }
        "message_stop" => Some(SseAction::Stop),
        _ => None,
    }
}

/// Parse raw SSE lines from a byte stream into `ChatCompletionChunk` items
fn parse_anthropic_sse(resp: reqwest::Response) -> ChatStream {
    // State: (response, line_buffer, event_type, msg_id, model)
    let stream = futures::stream::unfold(
        (resp, String::new(), String::new(), String::new(), String::new()),
        |(mut resp, mut buf, mut event_type, mut msg_id, mut model)| async move {
            loop {
                // Try to extract a complete line from the buffer.
                if let Some(newline_pos) = buf.find('\n') {
                    let line = buf[..newline_pos].trim_end_matches('\r').to_string();
                    buf = buf[newline_pos + 1..].to_string();

                    if line.is_empty() {
                        continue;
                    }

                    if let Some(rest) = line.strip_prefix("event: ") {
                        event_type = rest.to_string();
                        continue;
                    }

                    if let Some(json_str) = line.strip_prefix("data: ") {
                        let chunk = parse_sse_event(&event_type, json_str, &mut msg_id, &mut model);

                        if let Some(SseAction::Stop) = chunk {
                            return None;
                        }
                        if let Some(SseAction::Emit(c)) = chunk {
                            return Some((c, (resp, buf, event_type, msg_id, model)));
                        }
                        continue;
                    }

                    continue;
                }

                // Need more bytes from the network
                match resp.chunk().await {
                    Ok(Some(bytes)) => {
                        buf.push_str(&String::from_utf8_lossy(&bytes));
                    }
                    Ok(None) => {
                        return None;
                    }
                    Err(e) => {
                        let err: LLMError = LLMError::NetworkError(e.to_string());
                        return Some((Err(err), (resp, buf, event_type, msg_id, model)));
                    }
                }
            }
        },
    );

    Box::pin(stream)
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
        true
    }

    fn supports_tools(&self) -> bool {
        false
    }

    fn supports_vision(&self) -> bool {
        false
    }

    async fn chat(&self, request: ChatCompletionRequest) -> LLMResult<ChatCompletionResponse> {
        let body = self.build_request_body(&request, false);
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

    async fn chat_stream(&self, request: ChatCompletionRequest) -> LLMResult<ChatStream> {
        let body = self.build_request_body(&request, true);
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
        if !status.is_success() {
            let text = resp.text().await.map_err(Self::map_error)?;
            return Err(LLMError::ApiError {
                code: Some(status.as_u16().to_string()),
                message: text,
            });
        }

        Ok(parse_anthropic_sse(resp))
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
                streaming: true,
                tools: false,
                vision: false,
                json_mode: true,
                json_schema: false,
            },
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// processing a raw SSE text through parse_sse_event
    fn process_sse_text(raw: &str) -> Vec<ChatCompletionChunk> {
        let mut msg_id = String::new();
        let mut model = String::new();
        let mut event_type = String::new();
        let mut chunks = Vec::new();

        for line in raw.lines() {
            if line.is_empty() {
                continue;
            }
            if let Some(rest) = line.strip_prefix("event: ") {
                event_type = rest.to_string();
                continue;
            }
            if let Some(json_str) = line.strip_prefix("data: ") {
                match parse_sse_event(&event_type, json_str, &mut msg_id, &mut model) {
                    Some(SseAction::Emit(Ok(chunk))) => chunks.push(chunk),
                    Some(SseAction::Stop) => break,
                    _ => {}
                }
            }
        }
        chunks
    }

    #[test]
    fn test_config_defaults() {
        let config = AnthropicConfig::default();
        assert_eq!(config.base_url, "https://api.anthropic.com");
        assert_eq!(config.default_model, "claude-3.5-sonnet-20241022");
        assert_eq!(config.default_max_tokens, 4096);
    }

    #[test]
    fn test_config_builder() {
        let config = AnthropicConfig::new("sk-test")
            .with_model("claude-3-opus-20240229")
            .with_temperature(0.5)
            .with_max_tokens(1024)
            .with_base_url("https://custom.api.com")
            .with_version("2024-01-01")
            .with_timeout(120);

        assert_eq!(config.api_key, "sk-test");
        assert_eq!(config.default_model, "claude-3-opus-20240229");
        assert!((config.default_temperature - 0.5).abs() < f32::EPSILON);
        assert_eq!(config.default_max_tokens, 1024);
        assert_eq!(config.base_url, "https://custom.api.com");
        assert_eq!(config.version, "2024-01-01");
        assert_eq!(config.timeout_secs, 120);
    }

    #[test]
    fn test_provider_name() {
        let provider = AnthropicProvider::new("test-key");
        assert_eq!(provider.name(), "anthropic");
        assert!(provider.supports_streaming());
    }

    #[test]
    fn test_convert_messages_with_system() {
        let messages = vec![
            ChatMessage::system("You are helpful"),
            ChatMessage::user("Hello"),
        ];
        let (sys, converted) = AnthropicProvider::convert_messages(&messages);
        assert_eq!(sys, Some("You are helpful".to_string()));
        assert_eq!(converted.len(), 1);
        assert_eq!(converted[0]["role"], "user");
    }

    #[test]
    fn test_build_request_body_non_streaming() {
        let provider = AnthropicProvider::new("test-key");
        let request = ChatCompletionRequest::new("claude-3.5-sonnet-20241022")
            .user("Hello")
            .max_tokens(100);
        let body = provider.build_request_body(&request, false);
        assert_eq!(body.get("stream"), None);
        assert_eq!(body["max_tokens"], 100);
    }

    #[test]
    fn test_build_request_body_streaming() {
        let provider = AnthropicProvider::new("test-key");
        let request = ChatCompletionRequest::new("claude-3.5-sonnet-20241022")
            .user("Hello");
        let body = provider.build_request_body(&request, true);
        assert_eq!(body["stream"], true);
    }

    #[test]
    fn test_parse_sse_content_deltas() {
        let sse_data = "\
event: message_start
data: {\"type\":\"message_start\",\"message\":{\"id\":\"msg_123\",\"type\":\"message\",\"role\":\"assistant\",\"content\":[],\"model\":\"claude-3.5-sonnet-20241022\",\"stop_reason\":null,\"stop_sequence\":null,\"usage\":{\"input_tokens\":10,\"output_tokens\":1}}}

event: content_block_delta
data: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\"Hello\"}}

event: content_block_delta
data: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\" world\"}}

event: message_delta
data: {\"type\":\"message_delta\",\"delta\":{\"stop_reason\":\"end_turn\",\"stop_sequence\":null},\"usage\":{\"output_tokens\":5}}

event: message_stop
data: {\"type\":\"message_stop\"}";

        let chunks = process_sse_text(sse_data);
        assert_eq!(chunks.len(), 4);

        // message_start: role, usage
        assert_eq!(chunks[0].id, "msg_123");
        assert_eq!(chunks[0].choices[0].delta.role, Some(Role::Assistant));
        assert!(chunks[0].choices[0].delta.content.is_none());
        assert_eq!(chunks[0].usage.as_ref().unwrap().prompt_tokens, 10);

        // content deltas
        assert_eq!(chunks[1].choices[0].delta.content, Some("Hello".to_string()));
        assert!(chunks[1].choices[0].finish_reason.is_none());
        assert_eq!(chunks[2].choices[0].delta.content, Some(" world".to_string()));

        // message_delta: finish_reason + usage
        assert_eq!(chunks[3].choices[0].finish_reason, Some(FinishReason::Stop));
        assert_eq!(chunks[3].usage.as_ref().unwrap().completion_tokens, 5);
    }

    #[test]
    fn test_parse_sse_max_tokens_stop_reason() {
        let sse_data = "\
event: message_start
data: {\"type\":\"message_start\",\"message\":{\"id\":\"msg_456\",\"type\":\"message\",\"role\":\"assistant\",\"content\":[],\"model\":\"claude-3-opus\",\"stop_reason\":null,\"stop_sequence\":null,\"usage\":{\"input_tokens\":5,\"output_tokens\":0}}}

event: content_block_delta
data: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\"Truncated\"}}

event: message_delta
data: {\"type\":\"message_delta\",\"delta\":{\"stop_reason\":\"max_tokens\",\"stop_sequence\":null},\"usage\":{\"output_tokens\":100}}

event: message_stop
data: {\"type\":\"message_stop\"}";

        let chunks = process_sse_text(sse_data);
        assert_eq!(chunks.len(), 3);
        assert_eq!(chunks[2].choices[0].finish_reason, Some(FinishReason::Length));
    }

    #[test]
    fn test_parse_sse_empty_stream() {
        let sse_data = "\
event: message_stop
data: {\"type\":\"message_stop\"}";
        let chunks = process_sse_text(sse_data);
        assert!(chunks.is_empty());
    }

    #[test]
    fn test_parse_sse_event_unknown_type_skipped() {
        let mut msg_id = String::new();
        let mut model = String::new();
        let result = parse_sse_event("ping", "{}", &mut msg_id, &mut model);
        assert!(result.is_none());
    }

    #[test]
    fn test_parse_sse_event_message_start() {
        let mut msg_id = String::new();
        let mut model = String::new();
        let json = r#"{"type":"message_start","message":{"id":"msg_abc","type":"message","role":"assistant","content":[],"model":"claude-3","stop_reason":null,"stop_sequence":null,"usage":{"input_tokens":42,"output_tokens":0}}}"#;
        let result = parse_sse_event("message_start", json, &mut msg_id, &mut model);
        assert!(matches!(result, Some(SseAction::Emit(Ok(_)))));
        assert_eq!(msg_id, "msg_abc");
        assert_eq!(model, "claude-3");
        if let Some(SseAction::Emit(Ok(chunk))) = result {
            assert_eq!(chunk.usage.unwrap().prompt_tokens, 42);
        }
    }
}
