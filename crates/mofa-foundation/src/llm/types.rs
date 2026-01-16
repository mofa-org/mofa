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
    /// 函数调用结果（兼容旧 API）
    Function,
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
    /// 函数调用（旧 API）
    FunctionCall,
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
