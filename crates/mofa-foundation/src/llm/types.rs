//! LLM 核心类型定义
//! Core LLM type definitions
//!
//! 定义与 LLM 交互所需的所有类型
//! Defines all types required for interacting with LLMs

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use super::tool_schema;

// ============================================================================
// 消息类型
// Message Types
// ============================================================================

/// 消息角色
/// Message Role
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
#[derive(Default)]
pub enum Role {
    /// 系统消息（设置 LLM 行为）
    /// System message (configures LLM behavior)
    System,
    /// 用户消息
    /// User message
    #[default]
    User,
    /// 助手（LLM）响应
    /// Assistant (LLM) response
    Assistant,
    /// 工具调用结果
    /// Tool call result
    Tool,
}

/// 消息内容类型
/// Message content type
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ContentPart {
    /// 纯文本内容
    /// Plain text content
    Text { text: String },
    /// 图片内容
    /// Image content
    Image { image_url: ImageUrl },
    /// 音频内容
    /// Audio content
    Audio { audio: AudioData },
    /// 视频内容
    /// Video content
    Video { video: VideoData },
}

/// 图片 URL
/// Image URL
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageUrl {
    /// 图片 URL 或 base64 数据
    /// Image URL or base64 data
    pub url: String,
    /// 图片细节级别
    /// Image detail level
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<ImageDetail>,
}

/// 图片细节级别
/// Image detail level
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ImageDetail {
    Low,
    High,
    Auto,
}

/// 音频数据
/// Audio data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioData {
    /// base64 编码的音频数据
    /// base64 encoded audio data
    pub data: String,
    /// 音频格式
    /// Audio format
    pub format: String,
}

/// 视频数据
/// Video data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VideoData {
    /// base64 编码的视频数据
    /// base64 encoded video data
    pub data: String,
    /// 视频格式 (例如 mp4)
    /// Video format (e.g. mp4)
    pub format: String,
}

/// 聊天消息
/// Chat message
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    /// 消息角色
    /// Message role
    pub role: Role,
    /// 消息内容（可以是字符串或多部分内容）
    /// Message content (can be string or multipart content)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<MessageContent>,
    /// 消息名称（用于区分多个相同角色的消息）
    /// Message name (used to distinguish messages with the same role)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// 工具调用列表（仅 assistant 角色）
    /// List of tool calls (assistant role only)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<ToolCall>>,
    /// 工具调用 ID（仅 tool 角色）
    /// Tool call ID (tool role only)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
}

/// 消息内容（可以是简单字符串或多部分）
/// Message content (can be a simple string or multipart)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum MessageContent {
    /// 简单文本
    /// Simple text
    Text(String),
    /// 多部分内容（文本 + 图片等）
    /// Multipart content (text + images, etc.)
    Parts(Vec<ContentPart>),
}

impl ChatMessage {
    /// 创建系统消息
    /// Create a system message
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
    /// Create a user message
    pub fn user(content: impl Into<String>) -> Self {
        Self {
            role: Role::User,
            content: Some(MessageContent::Text(content.into())),
            name: None,
            tool_calls: None,
            tool_call_id: None,
        }
    }

    /// 创建用户消息（结构化内容）
    /// Create a user message (structured content)
    pub fn user_with_content(content: MessageContent) -> Self {
        Self {
            role: Role::User,
            content: Some(content),
            name: None,
            tool_calls: None,
            tool_call_id: None,
        }
    }

    /// 创建用户消息（多部分内容）
    /// Create a user message (multipart content)
    pub fn user_with_parts(parts: Vec<ContentPart>) -> Self {
        Self::user_with_content(MessageContent::Parts(parts))
    }

    /// 创建助手消息
    /// Create an assistant message
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
    /// Create an assistant message with tool calls
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
    /// Create a tool result message
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
    /// Create a user message with an image
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

    /// 创建带音频的用户消息
    /// Create a user message with audio
    pub fn user_with_audio(text: impl Into<String>, audio_base64: String, format: String) -> Self {
        Self {
            role: Role::User,
            content: Some(MessageContent::Parts(vec![
                ContentPart::Text { text: text.into() },
                ContentPart::Audio {
                    audio: AudioData {
                        data: audio_base64,
                        format,
                    },
                },
            ])),
            name: None,
            tool_calls: None,
            tool_call_id: None,
        }
    }

    /// 创建带视频的用户消息
    /// Create a user message with video
    pub fn user_with_video(text: impl Into<String>, video_base64: String, format: String) -> Self {
        Self {
            role: Role::User,
            content: Some(MessageContent::Parts(vec![
                ContentPart::Text { text: text.into() },
                ContentPart::Video {
                    video: VideoData {
                        data: video_base64,
                        format,
                    },
                },
            ])),
            name: None,
            tool_calls: None,
            tool_call_id: None,
        }
    }

    /// 获取文本内容
    /// Get text content
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
// Tool Definitions
// ============================================================================

/// 工具调用
/// Tool call
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    /// 工具调用 ID
    /// Tool call ID
    pub id: String,
    /// 工具类型
    /// Tool type
    #[serde(rename = "type")]
    pub call_type: String,
    /// 函数调用详情
    /// Function call details
    pub function: FunctionCall,
}

/// 函数调用
/// Function call
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunctionCall {
    /// 函数名称
    /// Function name
    pub name: String,
    /// 函数参数（JSON 字符串）
    /// Function arguments (JSON string)
    pub arguments: String,
}

/// 工具定义
/// Tool definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tool {
    /// 工具类型
    /// Tool type
    #[serde(rename = "type")]
    pub tool_type: String,
    /// 函数定义
    /// Function definition
    pub function: FunctionDefinition,
}

impl Tool {
    /// 创建函数工具
    /// Create a function tool
    pub fn function(
        name: impl Into<String>,
        description: impl Into<String>,
        parameters: serde_json::Value,
    ) -> Self {
        let parameters = tool_schema::normalize_schema(parameters);
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
/// Function definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunctionDefinition {
    /// 函数名称
    /// Function name
    pub name: String,
    /// 函数描述
    /// Function description
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// 参数 JSON Schema
    /// Parameters JSON Schema
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parameters: Option<serde_json::Value>,
    /// 是否严格模式
    /// Whether strict mode is enabled
    #[serde(skip_serializing_if = "Option::is_none")]
    pub strict: Option<bool>,
}

/// 工具选择策略
/// Tool choice strategy
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ToolChoice {
    /// 自动选择
    /// Automatic choice
    Auto,
    /// 不使用工具
    /// Use no tools
    None,
    /// 必须使用工具
    /// Tools required
    Required,
    /// 指定使用某个工具
    /// Specify a specific tool
    Specific {
        #[serde(rename = "type")]
        choice_type: String,
        function: ToolChoiceFunction,
    },
}

/// 工具选择函数
/// Tool choice function
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolChoiceFunction {
    pub name: String,
}

// ============================================================================
// 请求和响应
// Request and Response
// ============================================================================

/// Chat Completion 请求
/// Chat Completion Request
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ChatCompletionRequest {
    /// 模型名称
    /// Model name
    pub model: String,
    /// 消息列表
    /// Message list
    pub messages: Vec<ChatMessage>,
    /// 温度参数 (0.0 - 2.0)
    /// Temperature parameter (0.0 - 2.0)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    /// Top-p 采样
    /// Top-p sampling
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_p: Option<f32>,
    /// 生成的最大 token 数
    /// Maximum tokens to generate
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<u32>,
    /// 停止序列
    /// Stop sequences
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop: Option<Vec<String>>,
    /// 是否流式输出
    /// Whether to stream the output
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stream: Option<bool>,
    /// 可用工具列表
    /// Available tools list
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<Tool>>,
    /// 工具选择策略
    /// Tool choice strategy
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_choice: Option<ToolChoice>,
    /// 频率惩罚
    /// Frequency penalty
    #[serde(skip_serializing_if = "Option::is_none")]
    pub frequency_penalty: Option<f32>,
    /// 存在惩罚
    /// Presence penalty
    #[serde(skip_serializing_if = "Option::is_none")]
    pub presence_penalty: Option<f32>,
    /// 用户标识
    /// User identifier
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user: Option<String>,
    /// 响应格式
    /// Response format
    #[serde(skip_serializing_if = "Option::is_none")]
    pub response_format: Option<ResponseFormat>,
    /// 额外参数（用于不同提供商的特殊参数）
    /// Extra parameters (provider-specific special parameters)
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

impl ChatCompletionRequest {
    /// 创建新请求
    /// Create a new request
    pub fn new(model: impl Into<String>) -> Self {
        Self {
            model: model.into(),
            ..Default::default()
        }
    }

    /// 添加消息
    /// Add a message
    pub fn message(mut self, message: ChatMessage) -> Self {
        self.messages.push(message);
        self
    }

    /// 添加系统消息
    /// Add a system message
    pub fn system(mut self, content: impl Into<String>) -> Self {
        self.messages.push(ChatMessage::system(content));
        self
    }

    /// 添加用户消息
    /// Add a user message
    pub fn user(mut self, content: impl Into<String>) -> Self {
        self.messages.push(ChatMessage::user(content));
        self
    }

    /// 设置温度
    /// Set temperature
    pub fn temperature(mut self, temp: f32) -> Self {
        self.temperature = Some(temp);
        self
    }

    /// 设置最大 token 数
    /// Set maximum tokens
    pub fn max_tokens(mut self, tokens: u32) -> Self {
        self.max_tokens = Some(tokens);
        self
    }

    /// 添加工具
    /// Add a tool
    pub fn tool(mut self, tool: Tool) -> Self {
        self.tools.get_or_insert_with(Vec::new).push(tool);
        self
    }

    /// 设置工具列表
    /// Set tool list
    pub fn tools(mut self, tools: Vec<Tool>) -> Self {
        self.tools = Some(tools);
        self
    }

    /// 启用流式输出
    /// Enable streaming output
    pub fn stream(mut self) -> Self {
        self.stream = Some(true);
        self
    }
}

/// 响应格式
/// Response format
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResponseFormat {
    /// 格式类型
    /// Format type
    #[serde(rename = "type")]
    pub format_type: String,
    /// JSON Schema（用于 json_schema 类型）
    /// JSON Schema (used for json_schema type)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub json_schema: Option<serde_json::Value>,
}

impl ResponseFormat {
    /// 文本格式
    /// Text format
    pub fn text() -> Self {
        Self {
            format_type: "text".to_string(),
            json_schema: None,
        }
    }

    /// JSON 格式
    /// JSON format
    pub fn json() -> Self {
        Self {
            format_type: "json_object".to_string(),
            json_schema: None,
        }
    }

    /// JSON Schema 格式
    /// JSON Schema format
    pub fn json_schema(schema: serde_json::Value) -> Self {
        Self {
            format_type: "json_schema".to_string(),
            json_schema: Some(schema),
        }
    }
}

/// Chat Completion 响应
/// Chat Completion response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatCompletionResponse {
    /// 响应 ID
    /// Response ID
    pub id: String,
    /// 对象类型
    /// Object type
    pub object: String,
    /// 创建时间戳
    /// Creation timestamp
    pub created: u64,
    /// 模型名称
    /// Model name
    pub model: String,
    /// 选择列表
    /// List of choices
    pub choices: Vec<Choice>,
    /// 使用统计
    /// Usage statistics
    #[serde(skip_serializing_if = "Option::is_none")]
    pub usage: Option<Usage>,
    /// 系统指纹
    /// System fingerprint
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system_fingerprint: Option<String>,
}

impl ChatCompletionResponse {
    /// 获取第一个选择的消息内容
    /// Get the message content of the first choice
    pub fn content(&self) -> Option<&str> {
        self.choices.first()?.message.text_content()
    }

    /// 获取第一个选择的工具调用
    /// Get tool calls from the first choice
    pub fn tool_calls(&self) -> Option<&Vec<ToolCall>> {
        self.choices.first()?.message.tool_calls.as_ref()
    }

    /// 是否有工具调用
    /// Whether tool calls are present
    pub fn has_tool_calls(&self) -> bool {
        self.tool_calls().map(|t| !t.is_empty()).unwrap_or(false)
    }

    /// 获取完成原因
    /// Get the finish reason
    pub fn finish_reason(&self) -> Option<&FinishReason> {
        self.choices.first()?.finish_reason.as_ref()
    }
}

/// 选择
/// Choice
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Choice {
    /// 选择索引
    /// Choice index
    pub index: u32,
    /// 消息
    /// Message
    pub message: ChatMessage,
    /// 完成原因
    /// Finish reason
    #[serde(skip_serializing_if = "Option::is_none")]
    pub finish_reason: Option<FinishReason>,
    /// 日志概率
    /// Log probabilities
    #[serde(skip_serializing_if = "Option::is_none")]
    pub logprobs: Option<serde_json::Value>,
}

/// 完成原因
/// Finish reason
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FinishReason {
    /// 正常完成
    /// Completed normally
    Stop,
    /// 达到长度限制
    /// Length limit reached
    Length,
    /// 需要工具调用
    /// Tool calls required
    ToolCalls,
    /// 内容过滤
    /// Content filtered
    ContentFilter,
}

/// Token 使用统计
/// Token usage statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Usage {
    /// 提示 token 数
    /// Prompt tokens
    pub prompt_tokens: u32,
    /// 完成 token 数
    /// Completion tokens
    pub completion_tokens: u32,
    /// 总 token 数
    /// Total tokens
    pub total_tokens: u32,
}

/// LLM 响应元数据（用于事件处理器）
/// LLM response metadata (for event handlers)
///
/// 从 ChatCompletionResponse 或 ChatCompletionChunk 提取的关键元数据
/// Key metadata extracted from ChatCompletionResponse or ChatCompletionChunk
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LLMResponseMetadata {
    /// 响应 ID
    /// Response ID
    pub id: String,
    /// 模型名称
    /// Model name
    pub model: String,
    /// 提示 token 数
    /// Prompt tokens
    pub prompt_tokens: u32,
    /// 完成 token 数
    /// Completion tokens
    pub completion_tokens: u32,
    /// 总 token 数
    /// Total tokens
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
// Streaming Response
// ============================================================================

/// 流式响应块
/// Streaming response chunk
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatCompletionChunk {
    /// 响应 ID
    /// Response ID
    pub id: String,
    /// 对象类型
    /// Object type
    pub object: String,
    /// 创建时间戳
    /// Creation timestamp
    pub created: u64,
    /// 模型名称
    /// Model name
    pub model: String,
    /// 选择列表
    /// List of choices
    pub choices: Vec<ChunkChoice>,
    /// 使用统计（仅在最后一个块中）
    /// Usage statistics (only in the last chunk)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub usage: Option<Usage>,
}

/// 流式选择
/// Streaming choice
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChunkChoice {
    /// 选择索引
    /// Choice index
    pub index: u32,
    /// 增量内容
    /// Delta content
    pub delta: ChunkDelta,
    /// 完成原因
    /// Finish reason
    #[serde(skip_serializing_if = "Option::is_none")]
    pub finish_reason: Option<FinishReason>,
}

/// 增量内容
/// Incremental content
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ChunkDelta {
    /// 角色（仅在第一个块中）
    /// Role (only in the first chunk)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub role: Option<Role>,
    /// 内容增量
    /// Content delta
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    /// 工具调用增量
    /// Tool calls delta
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<ToolCallDelta>>,
}

/// 工具调用增量
/// Tool call delta
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCallDelta {
    /// 工具调用索引
    /// Tool call index
    pub index: u32,
    /// 工具调用 ID（仅在第一个块中）
    /// Tool call ID (only in the first chunk)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    /// 工具类型
    /// Tool type
    #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
    pub call_type: Option<String>,
    /// 函数调用增量
    /// Function call delta
    #[serde(skip_serializing_if = "Option::is_none")]
    pub function: Option<FunctionCallDelta>,
}

/// 函数调用增量
/// Function call delta
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunctionCallDelta {
    /// 函数名称（仅在第一个块中）
    /// Function name (only in the first chunk)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// 参数增量
    /// Arguments delta
    #[serde(skip_serializing_if = "Option::is_none")]
    pub arguments: Option<String>,
}

// ============================================================================
// Embedding
// ============================================================================

/// Embedding 请求
/// Embedding request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbeddingRequest {
    /// 模型名称
    /// Model name
    pub model: String,
    /// 输入文本（可以是单个字符串或字符串数组）
    /// Input text (can be a single string or an array of strings)
    pub input: EmbeddingInput,
    /// 编码格式
    /// Encoding format
    #[serde(skip_serializing_if = "Option::is_none")]
    pub encoding_format: Option<String>,
    /// 维度（部分模型支持）
    /// Dimensions (supported by some models)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dimensions: Option<u32>,
    /// 用户标识
    /// User identifier
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user: Option<String>,
}

/// Embedding 输入
/// Embedding input
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum EmbeddingInput {
    /// 单个字符串
    /// Single string
    Single(String),
    /// 字符串数组
    /// Array of strings
    Multiple(Vec<String>),
}

/// Embedding 响应
/// Embedding response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbeddingResponse {
    /// 对象类型
    /// Object type
    pub object: String,
    /// 模型名称
    /// Model name
    pub model: String,
    /// Embedding 数据列表
    /// List of embedding data
    pub data: Vec<EmbeddingData>,
    /// 使用统计
    /// Usage statistics
    pub usage: EmbeddingUsage,
}

/// Embedding 数据
/// Embedding data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbeddingData {
    /// 对象类型
    /// Object type
    pub object: String,
    /// 索引
    /// Index
    pub index: u32,
    /// Embedding 向量
    /// Embedding vector
    pub embedding: Vec<f32>,
}

/// Embedding 使用统计
/// Embedding usage statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbeddingUsage {
    /// 提示 token 数
    /// Prompt tokens
    pub prompt_tokens: u32,
    /// 总 token 数
    /// Total tokens
    pub total_tokens: u32,
}

// ============================================================================
// 错误类型
// Error Types
// ============================================================================

/// LLM 错误
/// LLM error
#[derive(Debug, Clone, thiserror::Error)]
pub enum LLMError {
    /// API 错误
    /// API error
    #[error("API error: {message} (code: {code:?})")]
    ApiError {
        code: Option<String>,
        message: String,
    },
    /// 认证错误
    /// Authentication error
    #[error("Authentication failed: {0}")]
    AuthError(String),
    /// 速率限制
    /// Rate limit exceeded
    #[error("Rate limited: {0}")]
    RateLimited(String),
    /// 配额超限
    /// Quota exceeded
    #[error("Quota exceeded: {0}")]
    QuotaExceeded(String),
    /// 模型不存在
    /// Model not found
    #[error("Model not found: {0}")]
    ModelNotFound(String),
    /// 上下文长度超限
    /// Context length exceeded
    #[error("Context length exceeded: {0}")]
    ContextLengthExceeded(String),
    /// 内容过滤
    /// Content filtered
    #[error("Content filtered: {0}")]
    ContentFiltered(String),
    /// 网络错误
    /// Network error
    #[error("Network error: {0}")]
    NetworkError(String),
    /// 超时
    /// Request timeout
    #[error("Request timeout: {0}")]
    Timeout(String),
    /// 序列化错误
    /// Serialization error
    #[error("Serialization error: {0}")]
    SerializationError(String),
    /// 配置错误
    /// Configuration error
    #[error("Configuration error: {0}")]
    ConfigError(String),
    /// 提供商不支持
    /// Provider not supported
    #[error("Provider not supported: {0}")]
    ProviderNotSupported(String),
    /// 其他错误
    /// Other error
    #[error("LLM error: {0}")]
    Other(String),
}

/// LLM 结果类型
/// LLM result type
pub type LLMResult<T> = Result<T, LLMError>;

// ============================================================================
// Retry Policy and Strategy
// ============================================================================

/// Retry strategy for LLM calls
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
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
    Linear {
        initial_delay_ms: u64,
        increment_ms: u64,
    },
    /// Exponential backoff
    Exponential {
        initial_delay_ms: u64,
        max_delay_ms: u64,
    },
    /// Exponential backoff with jitter
    ExponentialWithJitter {
        initial_delay_ms: u64,
        max_delay_ms: u64,
        jitter_ms: u64,
    },
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
            Self::Linear {
                initial_delay_ms,
                increment_ms,
            } => {
                let delay = *initial_delay_ms + (*increment_ms * attempt as u64);
                std::time::Duration::from_millis(delay)
            }
            Self::Exponential {
                initial_delay_ms,
                max_delay_ms,
            } => {
                let delay = *initial_delay_ms * 2u64.pow(attempt.min(10));
                let capped = delay.min(*max_delay_ms);
                std::time::Duration::from_millis(capped)
            }
            Self::ExponentialWithJitter {
                initial_delay_ms,
                max_delay_ms,
                jitter_ms,
            } => {
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
        map.insert(
            RetryableErrorType::Serialization,
            RetryStrategy::PromptRetry,
        );
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
        let strategy = BackoffStrategy::Linear {
            initial_delay_ms: 1000,
            increment_ms: 500,
        };
        assert_eq!(strategy.delay(0).as_millis(), 1000);
        assert_eq!(strategy.delay(1).as_millis(), 1500);
        assert_eq!(strategy.delay(2).as_millis(), 2000);
    }

    #[test]
    fn test_backoff_exponential_capping() {
        let strategy = BackoffStrategy::Exponential {
            initial_delay_ms: 1000,
            max_delay_ms: 5000,
        };
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
        assert!(
            delay >= 1800 && delay <= 2200,
            "Delay {} out of range",
            delay
        );
    }

    #[test]
    fn test_strategy_selection() {
        let policy = LLMRetryPolicy::default();

        // Serialization errors should use PromptRetry
        // Serialization errors should use PromptRetry
        let serde_err = LLMError::SerializationError("Invalid JSON".to_string());
        assert_eq!(
            policy.strategy_for_error(&serde_err),
            RetryStrategy::PromptRetry
        );

        // Network errors should use DirectRetry
        // Network errors should use DirectRetry
        let net_err = LLMError::NetworkError("Connection failed".to_string());
        assert_eq!(
            policy.strategy_for_error(&net_err),
            RetryStrategy::DirectRetry
        );
    }

    #[test]
    fn test_retryable_error_type_mapping() {
        let net_err = LLMError::NetworkError("error".to_string());
        assert_eq!(
            RetryableErrorType::from_error(&net_err),
            Some(RetryableErrorType::Network)
        );

        let rate_err = LLMError::RateLimited("error".to_string());
        assert_eq!(
            RetryableErrorType::from_error(&rate_err),
            Some(RetryableErrorType::RateLimit)
        );

        let auth_err = LLMError::AuthError("error".to_string());
        assert_eq!(
            RetryableErrorType::from_error(&auth_err),
            Some(RetryableErrorType::Authentication)
        );

        // Non-retryable errors return None
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
