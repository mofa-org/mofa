//! LLM 核心类型定义
//!
//! 定义与 LLM 交互所需的所有类型

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ============================================================================
// 消息类型
// ============================================================================

/// 消息角色
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
#[derive(Default)]
pub enum Role {
    /// 系统消息（设置 LLM 行为）
    System,
    /// 用户消息
    #[default]
    User,
    /// 助手（LLM）响应
    Assistant,
    /// 工具调用结果
    Tool,
}

/// 消息内容类型
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ContentPart {
    /// 纯文本内容
    Text { text: String },
    /// 图片内容
    Image { image_url: ImageUrl },
    /// 音频内容
    Audio { audio: AudioData },
}

/// 图片 URL
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageUrl {
    /// 图片 URL 或 base64 数据
    pub url: String,
    /// 图片细节级别
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<ImageDetail>,
}

/// 图片细节级别
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ImageDetail {
    Low,
    High,
    Auto,
}

/// 音频数据
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioData {
    /// base64 编码的音频数据
    pub data: String,
    /// 音频格式
    pub format: String,
}

/// 聊天消息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    /// 消息角色
    pub role: Role,
    /// 消息内容（可以是字符串或多部分内容）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<MessageContent>,
    /// 消息名称（用于区分多个相同角色的消息）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// 工具调用列表（仅 assistant 角色）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<ToolCall>>,
    /// 工具调用 ID（仅 tool 角色）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
}

/// 消息内容（可以是简单字符串或多部分）
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum MessageContent {
    /// 简单文本
    Text(String),
    /// 多部分内容（文本 + 图片等）
    Parts(Vec<ContentPart>),
}

impl ChatMessage {
    /// 创建系统消息
    pub fn system(content: impl Into<String>) -> Self {
        Self {
            role: Role::System,
            content: Some(MessageContent::Text(content.into())),
            name: None,
            tool_calls: None,
            tool_call_id: None,
        }
    }

    /// 创建用户消息
    pub fn user(content: impl Into<String>) -> Self {
        Self {
            role: Role::User,
            content: Some(MessageContent::Text(content.into())),
            name: None,
            tool_calls: None,
            tool_call_id: None,
        }
    }

    /// 创建助手消息
    pub fn assistant(content: impl Into<String>) -> Self {
        Self {
            role: Role::Assistant,
            content: Some(MessageContent::Text(content.into())),
            name: None,
            tool_calls: None,
            tool_call_id: None,
        }
    }

    /// 创建带工具调用的助手消息
    pub fn assistant_with_tool_calls(tool_calls: Vec<ToolCall>) -> Self {
        Self {
            role: Role::Assistant,
            content: None,
            name: None,
            tool_calls: Some(tool_calls),
            tool_call_id: None,
        }
    }

    /// 创建工具结果消息
    pub fn tool_result(tool_call_id: impl Into<String>, content: impl Into<String>) -> Self {
        Self {
            role: Role::Tool,
            content: Some(MessageContent::Text(content.into())),
            name: None,
            tool_calls: None,
            tool_call_id: Some(tool_call_id.into()),
        }
    }

    /// 创建带图片的用户消息
    pub fn user_with_image(text: impl Into<String>, image_url: impl Into<String>) -> Self {
        Self {
            role: Role::User,
            content: Some(MessageContent::Parts(vec![
                ContentPart::Text { text: text.into() },
                ContentPart::Image {
                    image_url: ImageUrl {
                        url: image_url.into(),
                        detail: None,
                    },
                },
            ])),
            name: None,
            tool_calls: None,
            tool_call_id: None,
        }
    }

    /// 获取文本内容
    pub fn text_content(&self) -> Option<&str> {
        match &self.content {
            Some(MessageContent::Text(s)) => Some(s),
            Some(MessageContent::Parts(parts)) => {
                for part in parts {
                    if let ContentPart::Text { text } = part {
                        return Some(text);
                    }
                }
                None
            }
            None => None,
        }
    }
}

// ============================================================================
// 工具定义
// ============================================================================

/// 工具调用
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    /// 工具调用 ID
    pub id: String,
    /// 工具类型
    #[serde(rename = "type")]
    pub call_type: String,
    /// 函数调用详情
    pub function: FunctionCall,
}

/// 函数调用
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunctionCall {
    /// 函数名称
    pub name: String,
    /// 函数参数（JSON 字符串）
    pub arguments: String,
}

/// 工具定义
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tool {
    /// 工具类型
    #[serde(rename = "type")]
    pub tool_type: String,
    /// 函数定义
    pub function: FunctionDefinition,
}

impl Tool {
    /// 创建函数工具
    pub fn function(
        name: impl Into<String>,
        description: impl Into<String>,
        parameters: serde_json::Value,
    ) -> Self {
        Self {
            tool_type: "function".to_string(),
            function: FunctionDefinition {
                name: name.into(),
                description: Some(description.into()),
                parameters: Some(parameters),
                strict: None,
            },
        }
    }
}

/// 函数定义
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunctionDefinition {
    /// 函数名称
    pub name: String,
    /// 函数描述
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// 参数 JSON Schema
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parameters: Option<serde_json::Value>,
    /// 是否严格模式
    #[serde(skip_serializing_if = "Option::is_none")]
    pub strict: Option<bool>,
}

/// 工具选择策略
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ToolChoice {
    /// 自动选择
    Auto,
    /// 不使用工具
    None,
    /// 必须使用工具
    Required,
    /// 指定使用某个工具
    Specific {
        #[serde(rename = "type")]
        choice_type: String,
        function: ToolChoiceFunction,
    },
}

/// 工具选择函数
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolChoiceFunction {
    pub name: String,
}

// ============================================================================
// 请求和响应
// ============================================================================

/// Chat Completion 请求
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ChatCompletionRequest {
    /// 模型名称
    pub model: String,
    /// 消息列表
    pub messages: Vec<ChatMessage>,
    /// 温度参数 (0.0 - 2.0)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    /// Top-p 采样
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_p: Option<f32>,
    /// 生成的最大 token 数
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<u32>,
    /// 停止序列
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop: Option<Vec<String>>,
    /// 是否流式输出
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stream: Option<bool>,
    /// 可用工具列表
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<Tool>>,
    /// 工具选择策略
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_choice: Option<ToolChoice>,
    /// 频率惩罚
    #[serde(skip_serializing_if = "Option::is_none")]
    pub frequency_penalty: Option<f32>,
    /// 存在惩罚
    #[serde(skip_serializing_if = "Option::is_none")]
    pub presence_penalty: Option<f32>,
    /// 用户标识
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user: Option<String>,
    /// 响应格式
    #[serde(skip_serializing_if = "Option::is_none")]
    pub response_format: Option<ResponseFormat>,
    /// 额外参数（用于不同提供商的特殊参数）
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

impl ChatCompletionRequest {
    /// 创建新请求
    pub fn new(model: impl Into<String>) -> Self {
        Self {
            model: model.into(),
            ..Default::default()
        }
    }

    /// 添加消息
    pub fn message(mut self, message: ChatMessage) -> Self {
        self.messages.push(message);
        self
    }

    /// 添加系统消息
    pub fn system(mut self, content: impl Into<String>) -> Self {
        self.messages.push(ChatMessage::system(content));
        self
    }

    /// 添加用户消息
    pub fn user(mut self, content: impl Into<String>) -> Self {
        self.messages.push(ChatMessage::user(content));
        self
    }

    /// 设置温度
    pub fn temperature(mut self, temp: f32) -> Self {
        self.temperature = Some(temp);
        self
    }

    /// 设置最大 token 数
    pub fn max_tokens(mut self, tokens: u32) -> Self {
        self.max_tokens = Some(tokens);
        self
    }

    /// 添加工具
    pub fn tool(mut self, tool: Tool) -> Self {
        self.tools.get_or_insert_with(Vec::new).push(tool);
        self
    }

    /// 设置工具列表
    pub fn tools(mut self, tools: Vec<Tool>) -> Self {
        self.tools = Some(tools);
        self
    }

    /// 启用流式输出
    pub fn stream(mut self) -> Self {
        self.stream = Some(true);
        self
    }
}

/// 响应格式
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResponseFormat {
    /// 格式类型
    #[serde(rename = "type")]
    pub format_type: String,
    /// JSON Schema（用于 json_schema 类型）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub json_schema: Option<serde_json::Value>,
}

impl ResponseFormat {
    /// 文本格式
    pub fn text() -> Self {
        Self {
            format_type: "text".to_string(),
            json_schema: None,
        }
    }

    /// JSON 格式
    pub fn json() -> Self {
        Self {
            format_type: "json_object".to_string(),
            json_schema: None,
        }
    }

    /// JSON Schema 格式
    pub fn json_schema(schema: serde_json::Value) -> Self {
        Self {
            format_type: "json_schema".to_string(),
            json_schema: Some(schema),
        }
    }
}

/// Chat Completion 响应
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatCompletionResponse {
    /// 响应 ID
    pub id: String,
    /// 对象类型
    pub object: String,
    /// 创建时间戳
    pub created: u64,
    /// 模型名称
    pub model: String,
    /// 选择列表
    pub choices: Vec<Choice>,
    /// 使用统计
    #[serde(skip_serializing_if = "Option::is_none")]
    pub usage: Option<Usage>,
    /// 系统指纹
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system_fingerprint: Option<String>,
}

impl ChatCompletionResponse {
    /// 获取第一个选择的消息内容
    pub fn content(&self) -> Option<&str> {
        self.choices.first()?.message.text_content()
    }

    /// 获取第一个选择的工具调用
    pub fn tool_calls(&self) -> Option<&Vec<ToolCall>> {
        self.choices.first()?.message.tool_calls.as_ref()
    }

    /// 是否有工具调用
    pub fn has_tool_calls(&self) -> bool {
        self.tool_calls().map(|t| !t.is_empty()).unwrap_or(false)
    }

    /// 获取完成原因
    pub fn finish_reason(&self) -> Option<&FinishReason> {
        self.choices.first()?.finish_reason.as_ref()
    }
}

/// 选择
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Choice {
    /// 选择索引
    pub index: u32,
    /// 消息
    pub message: ChatMessage,
    /// 完成原因
    #[serde(skip_serializing_if = "Option::is_none")]
    pub finish_reason: Option<FinishReason>,
    /// 日志概率
    #[serde(skip_serializing_if = "Option::is_none")]
    pub logprobs: Option<serde_json::Value>,
}

/// 完成原因
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FinishReason {
    /// 正常完成
    Stop,
    /// 达到长度限制
    Length,
    /// 需要工具调用
    ToolCalls,
    /// 内容过滤
    ContentFilter,
}

/// Token 使用统计
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Usage {
    /// 提示 token 数
    pub prompt_tokens: u32,
    /// 完成 token 数
    pub completion_tokens: u32,
    /// 总 token 数
    pub total_tokens: u32,
}

/// LLM 响应元数据（用于事件处理器）
///
/// 从 ChatCompletionResponse 或 ChatCompletionChunk 提取的关键元数据
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LLMResponseMetadata {
    /// 响应 ID
    pub id: String,
    /// 模型名称
    pub model: String,
    /// 提示 token 数
    pub prompt_tokens: u32,
    /// 完成 token 数
    pub completion_tokens: u32,
    /// 总 token 数
    pub total_tokens: u32,
}

impl From<&ChatCompletionResponse> for LLMResponseMetadata {
    fn from(resp: &ChatCompletionResponse) -> Self {
        let usage = resp.usage.as_ref();
        Self {
            id: resp.id.clone(),
            model: resp.model.clone(),
            prompt_tokens: usage.map(|u| u.prompt_tokens).unwrap_or(0),
            completion_tokens: usage.map(|u| u.completion_tokens).unwrap_or(0),
            total_tokens: usage.map(|u| u.total_tokens).unwrap_or(0),
        }
    }
}

impl From<&ChatCompletionChunk> for LLMResponseMetadata {
    fn from(chunk: &ChatCompletionChunk) -> Self {
        let usage = chunk.usage.as_ref();
        Self {
            id: chunk.id.clone(),
            model: chunk.model.clone(),
            prompt_tokens: usage.map(|u| u.prompt_tokens).unwrap_or(0),
            completion_tokens: usage.map(|u| u.completion_tokens).unwrap_or(0),
            total_tokens: usage.map(|u| u.total_tokens).unwrap_or(0),
        }
    }
}

// ============================================================================
// 流式响应
// ============================================================================

/// 流式响应块
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatCompletionChunk {
    /// 响应 ID
    pub id: String,
    /// 对象类型
    pub object: String,
    /// 创建时间戳
    pub created: u64,
    /// 模型名称
    pub model: String,
    /// 选择列表
    pub choices: Vec<ChunkChoice>,
    /// 使用统计（仅在最后一个块中）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub usage: Option<Usage>,
}

/// 流式选择
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChunkChoice {
    /// 选择索引
    pub index: u32,
    /// 增量内容
    pub delta: ChunkDelta,
    /// 完成原因
    #[serde(skip_serializing_if = "Option::is_none")]
    pub finish_reason: Option<FinishReason>,
}

/// 增量内容
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ChunkDelta {
    /// 角色（仅在第一个块中）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub role: Option<Role>,
    /// 内容增量
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    /// 工具调用增量
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<ToolCallDelta>>,
}

/// 工具调用增量
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCallDelta {
    /// 工具调用索引
    pub index: u32,
    /// 工具调用 ID（仅在第一个块中）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    /// 工具类型
    #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
    pub call_type: Option<String>,
    /// 函数调用增量
    #[serde(skip_serializing_if = "Option::is_none")]
    pub function: Option<FunctionCallDelta>,
}

/// 函数调用增量
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunctionCallDelta {
    /// 函数名称（仅在第一个块中）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// 参数增量
    #[serde(skip_serializing_if = "Option::is_none")]
    pub arguments: Option<String>,
}

// ============================================================================
// Embedding
// ============================================================================

/// Embedding 请求
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbeddingRequest {
    /// 模型名称
    pub model: String,
    /// 输入文本（可以是单个字符串或字符串数组）
    pub input: EmbeddingInput,
    /// 编码格式
    #[serde(skip_serializing_if = "Option::is_none")]
    pub encoding_format: Option<String>,
    /// 维度（部分模型支持）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dimensions: Option<u32>,
    /// 用户标识
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user: Option<String>,
}

/// Embedding 输入
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum EmbeddingInput {
    /// 单个字符串
    Single(String),
    /// 字符串数组
    Multiple(Vec<String>),
}

/// Embedding 响应
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbeddingResponse {
    /// 对象类型
    pub object: String,
    /// 模型名称
    pub model: String,
    /// Embedding 数据列表
    pub data: Vec<EmbeddingData>,
    /// 使用统计
    pub usage: EmbeddingUsage,
}

/// Embedding 数据
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbeddingData {
    /// 对象类型
    pub object: String,
    /// 索引
    pub index: u32,
    /// Embedding 向量
    pub embedding: Vec<f32>,
}

/// Embedding 使用统计
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbeddingUsage {
    /// 提示 token 数
    pub prompt_tokens: u32,
    /// 总 token 数
    pub total_tokens: u32,
}

// ============================================================================
// 错误类型
// ============================================================================

/// LLM 错误
#[derive(Debug, Clone, thiserror::Error)]
pub enum LLMError {
    /// API 错误
    #[error("API error: {message} (code: {code:?})")]
    ApiError {
        code: Option<String>,
        message: String,
    },
    /// 认证错误
    #[error("Authentication failed: {0}")]
    AuthError(String),
    /// 速率限制
    #[error("Rate limited: {0}")]
    RateLimited(String),
    /// 配额超限
    #[error("Quota exceeded: {0}")]
    QuotaExceeded(String),
    /// 模型不存在
    #[error("Model not found: {0}")]
    ModelNotFound(String),
    /// 上下文长度超限
    #[error("Context length exceeded: {0}")]
    ContextLengthExceeded(String),
    /// 内容过滤
    #[error("Content filtered: {0}")]
    ContentFiltered(String),
    /// 网络错误
    #[error("Network error: {0}")]
    NetworkError(String),
    /// 超时
    #[error("Request timeout: {0}")]
    Timeout(String),
    /// 序列化错误
    #[error("Serialization error: {0}")]
    SerializationError(String),
    /// 配置错误
    #[error("Configuration error: {0}")]
    ConfigError(String),
    /// 提供商不支持
    #[error("Provider not supported: {0}")]
    ProviderNotSupported(String),
    /// 其他错误
    #[error("LLM error: {0}")]
    Other(String),
}

/// LLM 结果类型
pub type LLMResult<T> = Result<T, LLMError>;

// ============================================================================
// Retry Policy and Strategy
// ============================================================================

/// Retry strategy for LLM calls
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[derive(Default)]
pub enum RetryStrategy {
    /// Fail immediately without retry
    NoRetry,
    /// Simple retry without prompt modification
    #[default]
    DirectRetry,
    /// Append error context to system prompt (best for JSON errors)
    PromptRetry,
}


/// Backoff strategy for retry delays
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum BackoffStrategy {
    /// Fixed delay between retries
    Fixed { delay_ms: u64 },
    /// Linear backoff with increment
    Linear { initial_delay_ms: u64, increment_ms: u64 },
    /// Exponential backoff
    Exponential { initial_delay_ms: u64, max_delay_ms: u64 },
    /// Exponential backoff with jitter
    ExponentialWithJitter { initial_delay_ms: u64, max_delay_ms: u64, jitter_ms: u64 },
}

impl Default for BackoffStrategy {
    fn default() -> Self {
        Self::ExponentialWithJitter {
            initial_delay_ms: 1000,
            max_delay_ms: 30000,
            jitter_ms: 500,
        }
    }
}

impl BackoffStrategy {
    /// Calculate delay duration for a given attempt (0-indexed)
    pub fn delay(&self, attempt: u32) -> std::time::Duration {
        match self {
            Self::Fixed { delay_ms } => std::time::Duration::from_millis(*delay_ms),
            Self::Linear { initial_delay_ms, increment_ms } => {
                let delay = *initial_delay_ms + (*increment_ms * attempt as u64);
                std::time::Duration::from_millis(delay)
            }
            Self::Exponential { initial_delay_ms, max_delay_ms } => {
                let delay = *initial_delay_ms * 2u64.pow(attempt.min(10));
                let capped = delay.min(*max_delay_ms);
                std::time::Duration::from_millis(capped)
            }
            Self::ExponentialWithJitter { initial_delay_ms, max_delay_ms, jitter_ms } => {
                let base_delay = *initial_delay_ms * 2u64.pow(attempt.min(10));
                let capped = base_delay.min(*max_delay_ms);
                let jitter = if *jitter_ms > 0 {
                    use rand::Rng;
                    let mut rng = rand::thread_rng();
                    rng.gen_range(0..*jitter_ms) as i64 - (*jitter_ms as i64 / 2)
                } else {
                    0
                };
                let final_delay = (capped as i64 + jitter).max(0) as u64;
                std::time::Duration::from_millis(final_delay)
            }
        }
    }
}

/// Error types that trigger retry
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum RetryableErrorType {
    /// Network-related errors
    Network,
    /// Rate limit errors
    RateLimit,
    /// Serialization errors (including JSON parsing)
    Serialization,
    /// Authentication errors
    Authentication,
    /// Server errors (5xx)
    ServerError,
    /// Context length exceeded
    ContextLength,
    /// Content filtered
    ContentFiltered,
}

impl RetryableErrorType {
    /// Determine if an error is retryable and what type it is
    pub fn from_error(error: &LLMError) -> Option<Self> {
        match error {
            LLMError::NetworkError(_) => Some(Self::Network),
            LLMError::Timeout(_) => Some(Self::Network),
            LLMError::RateLimited(_) => Some(Self::RateLimit),
            LLMError::SerializationError(_) => Some(Self::Serialization),
            LLMError::AuthError(_) => Some(Self::Authentication),
            LLMError::ApiError { code, .. } => {
                if let Some(c) = code {
                    // Check for 5xx server errors
                    if c.starts_with('5') {
                        return Some(Self::ServerError);
                    }
                }
                None
            }
            LLMError::ContextLengthExceeded(_) => Some(Self::ContextLength),
            LLMError::ContentFiltered(_) => Some(Self::ContentFiltered),
            // Non-retryable errors
            LLMError::QuotaExceeded(_)
            | LLMError::ModelNotFound(_)
            | LLMError::ConfigError(_)
            | LLMError::ProviderNotSupported(_)
            | LLMError::Other(_) => None,
        }
    }
}

/// Retry policy for LLM calls
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LLMRetryPolicy {
    /// Maximum number of attempts (including the first attempt)
    pub max_attempts: u32,
    /// Backoff strategy for delays
    pub backoff: BackoffStrategy,
    /// Default retry strategy
    pub default_strategy: RetryStrategy,
    /// Per-error-type strategies
    pub error_strategies: std::collections::HashMap<RetryableErrorType, RetryStrategy>,
    /// Error types that should trigger retry
    pub retry_on: Vec<RetryableErrorType>,
}

impl Default for LLMRetryPolicy {
    fn default() -> Self {
        Self {
            max_attempts: 3,
            backoff: BackoffStrategy::default(),
            default_strategy: RetryStrategy::PromptRetry,
            error_strategies: Self::default_strategies(),
            retry_on: vec![
                RetryableErrorType::Network,
                RetryableErrorType::RateLimit,
                RetryableErrorType::Serialization,
                RetryableErrorType::ServerError,
            ],
        }
    }
}

impl LLMRetryPolicy {
    fn default_strategies() -> std::collections::HashMap<RetryableErrorType, RetryStrategy> {
        let mut map = std::collections::HashMap::new();
        // PromptRetry is best for serialization errors (e.g., JSON format issues)
        map.insert(RetryableErrorType::Serialization, RetryStrategy::PromptRetry);
        // Direct retry for transient errors
        map.insert(RetryableErrorType::Network, RetryStrategy::DirectRetry);
        map.insert(RetryableErrorType::RateLimit, RetryStrategy::DirectRetry);
        map.insert(RetryableErrorType::ServerError, RetryStrategy::DirectRetry);
        map.insert(RetryableErrorType::Authentication, RetryStrategy::NoRetry);
        map.insert(RetryableErrorType::ContextLength, RetryStrategy::NoRetry);
        map.insert(RetryableErrorType::ContentFiltered, RetryStrategy::NoRetry);
        map
    }

    /// Create a policy with no retry
    pub fn no_retry() -> Self {
        Self {
            max_attempts: 1,
            backoff: BackoffStrategy::Fixed { delay_ms: 0 },
            default_strategy: RetryStrategy::NoRetry,
            error_strategies: Self::default_strategies(),
            retry_on: vec![],
        }
    }

    /// Create a policy with custom max attempts
    pub fn with_max_attempts(max: u32) -> Self {
        Self {
            max_attempts: max.max(1),
            ..Default::default()
        }
    }

    /// Set custom backoff strategy
    pub fn with_backoff(mut self, backoff: BackoffStrategy) -> Self {
        self.backoff = backoff;
        self
    }

    /// Get the retry strategy for a specific error
    pub fn strategy_for_error(&self, error: &LLMError) -> RetryStrategy {
        RetryableErrorType::from_error(error)
            .and_then(|error_type| self.error_strategies.get(&error_type).cloned())
            .unwrap_or_else(|| self.default_strategy.clone())
    }

    /// Check if an error should trigger retry
    pub fn should_retry_error(&self, error: &LLMError) -> bool {
        if let Some(error_type) = RetryableErrorType::from_error(error) {
            self.retry_on.contains(&error_type)
        } else {
            false
        }
    }
}

/// JSON validation error details
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JSONValidationError {
    /// Raw content that failed to parse
    pub raw_content: String,
    /// Parse error message
    pub parse_error: String,
    /// Expected JSON schema (if available)
    pub expected_schema: Option<serde_json::Value>,
}

impl std::fmt::Display for JSONValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "JSON validation failed: {}. Raw content (first 200 chars): {}",
            self.parse_error,
            self.raw_content.chars().take(200).collect::<String>()
        )
    }
}

impl std::error::Error for JSONValidationError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_backoff_fixed() {
        let strategy = BackoffStrategy::Fixed { delay_ms: 1000 };
        assert_eq!(strategy.delay(0).as_millis(), 1000);
        assert_eq!(strategy.delay(5).as_millis(), 1000);
    }

    #[test]
    fn test_backoff_linear() {
        let strategy = BackoffStrategy::Linear { initial_delay_ms: 1000, increment_ms: 500 };
        assert_eq!(strategy.delay(0).as_millis(), 1000);
        assert_eq!(strategy.delay(1).as_millis(), 1500);
        assert_eq!(strategy.delay(2).as_millis(), 2000);
    }

    #[test]
    fn test_backoff_exponential_capping() {
        let strategy = BackoffStrategy::Exponential { initial_delay_ms: 1000, max_delay_ms: 5000 };
        assert_eq!(strategy.delay(0).as_millis(), 1000);
        assert_eq!(strategy.delay(1).as_millis(), 2000);
        assert_eq!(strategy.delay(2).as_millis(), 4000);
        assert_eq!(strategy.delay(3).as_millis(), 5000); // Capped
        assert_eq!(strategy.delay(10).as_millis(), 5000); // Capped
    }

    #[test]
    fn test_backoff_jitter_range() {
        let strategy = BackoffStrategy::ExponentialWithJitter {
            initial_delay_ms: 1000,
            max_delay_ms: 10000,
            jitter_ms: 200,
        };
        // Jitter should keep delay within reasonable bounds
        let delay = strategy.delay(1).as_millis();
        assert!(delay >= 1800 && delay <= 2200, "Delay {} out of range", delay);
    }

    #[test]
    fn test_strategy_selection() {
        let policy = LLMRetryPolicy::default();

        // Serialization errors should use PromptRetry
        let serde_err = LLMError::SerializationError("Invalid JSON".to_string());
        assert_eq!(policy.strategy_for_error(&serde_err), RetryStrategy::PromptRetry);

        // Network errors should use DirectRetry
        let net_err = LLMError::NetworkError("Connection failed".to_string());
        assert_eq!(policy.strategy_for_error(&net_err), RetryStrategy::DirectRetry);
    }

    #[test]
    fn test_retryable_error_type_mapping() {
        let net_err = LLMError::NetworkError("error".to_string());
        assert_eq!(RetryableErrorType::from_error(&net_err), Some(RetryableErrorType::Network));

        let rate_err = LLMError::RateLimited("error".to_string());
        assert_eq!(RetryableErrorType::from_error(&rate_err), Some(RetryableErrorType::RateLimit));

        let auth_err = LLMError::AuthError("error".to_string());
        assert_eq!(RetryableErrorType::from_error(&auth_err), Some(RetryableErrorType::Authentication));

        // Non-retryable errors return None
        let quota_err = LLMError::QuotaExceeded("error".to_string());
        assert_eq!(RetryableErrorType::from_error(&quota_err), None);
    }

    #[test]
    fn test_no_retry_policy() {
        let policy = LLMRetryPolicy::no_retry();
        assert_eq!(policy.max_attempts, 1);
        assert!(!policy.should_retry_error(&LLMError::NetworkError("error".to_string())));
    }
}
