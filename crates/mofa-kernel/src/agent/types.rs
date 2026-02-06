//! Agent 核心类型定义
//!
//! 定义统一的 Agent 输入、输出和状态类型

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;

// 导出统一类型模块
pub mod global;
pub mod event;
pub mod error;

pub use error::{ErrorCategory, ErrorContext, GlobalError, GlobalResult};
pub use event::{execution, lifecycle, message, plugin, state};
pub use event::{EventBuilder, GlobalEvent};
// 重新导出常用类型
pub use global::{MessageContent, MessageMetadata, GlobalMessage};

// ============================================================================
// Agent 状态
// ============================================================================

/// Agent 状态机
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[derive(Default)]
pub enum AgentState {
    /// 已创建，未初始化
    #[default]
    Created,
    /// 正在初始化
    Initializing,
    /// 就绪，可执行
    Ready,
    /// 运行中
    Running,
    /// 正在执行
    Executing,
    /// 已暂停
    Paused,
    /// 已中断
    Interrupted,
    /// 正在关闭
    ShuttingDown,
    /// 已终止/关闭
    Shutdown,
    /// 失败状态
    Failed,
    /// 销毁
    Destroyed,
    /// 错误状态 (带消息)
    Error(String),
}


impl fmt::Display for AgentState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AgentState::Created => write!(f, "Created"),
            AgentState::Initializing => write!(f, "Initializing"),
            AgentState::Ready => write!(f, "Ready"),
            AgentState::Executing => write!(f, "Executing"),
            AgentState::Paused => write!(f, "Paused"),
            AgentState::Interrupted => write!(f, "Interrupted"),
            AgentState::ShuttingDown => write!(f, "ShuttingDown"),
            AgentState::Shutdown => write!(f, "Shutdown"),
            AgentState::Failed => write!(f, "Failed"),
            AgentState::Error(msg) => write!(f, "Error({})", msg),
            AgentState::Running => {write!(f, "Running")}
            AgentState::Destroyed => {write!(f, "Destroyed")}
        }
    }
}

impl AgentState {
    /// 转换到目标状态
    pub fn transition_to(&self, target: AgentState) -> Result<AgentState, super::error::AgentError> {
        if self.can_transition_to(&target) {
            Ok(target)
        } else {
            Err(super::error::AgentError::invalid_state_transition(self, &target))
        }
    }

    /// 检查是否可以转换到目标状态
    pub fn can_transition_to(&self, target: &AgentState) -> bool {
        use AgentState::*;
        matches!(
            (self, target),
            (Created, Initializing)
                | (Initializing, Ready)
                | (Initializing, Error(_))
                | (Initializing, Failed)
                | (Ready, Executing)
                | (Ready, ShuttingDown)
                | (Executing, Ready)
                | (Executing, Paused)
                | (Executing, Interrupted)
                | (Executing, Error(_))
                | (Executing, Failed)
                | (Paused, Ready)
                | (Paused, Executing)
                | (Paused, ShuttingDown)
                | (Interrupted, Ready)
                | (Interrupted, ShuttingDown)
                | (ShuttingDown, Shutdown)
                | (Error(_), ShuttingDown)
                | (Error(_), Shutdown)
                | (Failed, ShuttingDown)
                | (Failed, Shutdown)
        )
    }

    /// 是否为活动状态
    pub fn is_active(&self) -> bool {
        matches!(self, AgentState::Ready | AgentState::Executing)
    }

    /// 是否为终止状态
    pub fn is_terminal(&self) -> bool {
        matches!(self, AgentState::Shutdown | AgentState::Failed | AgentState::Error(_))
    }
}

// ============================================================================
// Agent 输入
// ============================================================================

/// Agent 输入类型
#[derive(Debug, Clone, Serialize, Deserialize)]
#[derive(Default)]
pub enum AgentInput {
    /// 文本输入
    Text(String),
    /// 多行文本
    Texts(Vec<String>),
    /// 结构化 JSON
    Json(serde_json::Value),
    /// 键值对
    Map(HashMap<String, serde_json::Value>),
    /// 二进制数据
    Binary(Vec<u8>),
    /// 空输入
    #[default]
    Empty,
}


impl AgentInput {
    /// 创建文本输入
    pub fn text(s: impl Into<String>) -> Self {
        Self::Text(s.into())
    }

    /// 创建 JSON 输入
    pub fn json(value: serde_json::Value) -> Self {
        Self::Json(value)
    }

    /// 创建键值对输入
    pub fn map(map: HashMap<String, serde_json::Value>) -> Self {
        Self::Map(map)
    }

    /// 获取文本内容
    pub fn as_text(&self) -> Option<&str> {
        match self {
            Self::Text(s) => Some(s),
            _ => None,
        }
    }

    /// 转换为文本
    pub fn to_text(&self) -> String {
        match self {
            Self::Text(s) => s.clone(),
            Self::Texts(v) => v.join("\n"),
            Self::Json(v) => v.to_string(),
            Self::Map(m) => serde_json::to_string(m).unwrap_or_default(),
            Self::Binary(b) => String::from_utf8_lossy(b).to_string(),
            Self::Empty => String::new(),
        }
    }

    /// 获取 JSON 内容
    pub fn as_json(&self) -> Option<&serde_json::Value> {
        match self {
            Self::Json(v) => Some(v),
            _ => None,
        }
    }

    /// 转换为 JSON
    pub fn to_json(&self) -> serde_json::Value {
        match self {
            Self::Text(s) => serde_json::Value::String(s.clone()),
            Self::Texts(v) => serde_json::json!(v),
            Self::Json(v) => v.clone(),
            Self::Map(m) => serde_json::to_value(m).unwrap_or_default(),
            Self::Binary(b) => serde_json::json!({ "binary": base64_encode(b) }),
            Self::Empty => serde_json::Value::Null,
        }
    }

    /// 是否为空
    pub fn is_empty(&self) -> bool {
        matches!(self, Self::Empty)
    }
}

impl From<String> for AgentInput {
    fn from(s: String) -> Self {
        Self::Text(s)
    }
}

impl From<&str> for AgentInput {
    fn from(s: &str) -> Self {
        Self::Text(s.to_string())
    }
}

impl From<serde_json::Value> for AgentInput {
    fn from(v: serde_json::Value) -> Self {
        Self::Json(v)
    }
}

// ============================================================================
// Agent 输出
// ============================================================================

/// Agent 输出类型
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentOutput {
    /// 主输出内容
    pub content: OutputContent,
    /// 输出元数据
    pub metadata: HashMap<String, serde_json::Value>,
    /// 使用的工具
    pub tools_used: Vec<ToolUsage>,
    /// 推理步骤 (如果有)
    pub reasoning_steps: Vec<ReasoningStep>,
    /// 执行时间 (毫秒)
    pub duration_ms: u64,
    /// Token 使用统计
    pub token_usage: Option<TokenUsage>,
}

impl Default for AgentOutput {
    fn default() -> Self {
        Self {
            content: OutputContent::Empty,
            metadata: HashMap::new(),
            tools_used: Vec::new(),
            reasoning_steps: Vec::new(),
            duration_ms: 0,
            token_usage: None,
        }
    }
}

impl AgentOutput {
    /// 创建文本输出
    pub fn text(s: impl Into<String>) -> Self {
        Self {
            content: OutputContent::Text(s.into()),
            ..Default::default()
        }
    }

    /// 创建 JSON 输出
    pub fn json(value: serde_json::Value) -> Self {
        Self {
            content: OutputContent::Json(value),
            ..Default::default()
        }
    }

    /// 创建错误输出
    pub fn error(message: impl Into<String>) -> Self {
        Self {
            content: OutputContent::Error(message.into()),
            ..Default::default()
        }
    }

    /// 获取文本内容
    pub fn as_text(&self) -> Option<&str> {
        match &self.content {
            OutputContent::Text(s) => Some(s),
            _ => None,
        }
    }

    /// 转换为文本
    pub fn to_text(&self) -> String {
        self.content.to_text()
    }

    /// 设置执行时间
    pub fn with_duration(mut self, duration_ms: u64) -> Self {
        self.duration_ms = duration_ms;
        self
    }

    /// 添加元数据
    pub fn with_metadata(mut self, key: impl Into<String>, value: serde_json::Value) -> Self {
        self.metadata.insert(key.into(), value);
        self
    }

    /// 添加工具使用记录
    pub fn with_tool_usage(mut self, usage: ToolUsage) -> Self {
        self.tools_used.push(usage);
        self
    }

    /// 设置所有工具使用记录
    pub fn with_tools_used(mut self, usages: Vec<ToolUsage>) -> Self {
        self.tools_used = usages;
        self
    }

    /// 添加推理步骤
    pub fn with_reasoning_step(mut self, step: ReasoningStep) -> Self {
        self.reasoning_steps.push(step);
        self
    }

    /// 设置所有推理步骤
    pub fn with_reasoning_steps(mut self, steps: Vec<ReasoningStep>) -> Self {
        self.reasoning_steps = steps;
        self
    }

    /// 设置 Token 使用
    pub fn with_token_usage(mut self, usage: TokenUsage) -> Self {
        self.token_usage = Some(usage);
        self
    }

    /// 是否为错误
    pub fn is_error(&self) -> bool {
        matches!(self.content, OutputContent::Error(_))
    }
}

/// 输出内容类型
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum OutputContent {
    /// 文本输出
    Text(String),
    /// 多行文本
    Texts(Vec<String>),
    /// JSON 输出
    Json(serde_json::Value),
    /// 二进制输出
    Binary(Vec<u8>),
    /// 流式输出标记
    Stream,
    /// 错误输出
    Error(String),
    /// 空输出
    Empty,
}

impl OutputContent {
    /// 转换为文本
    pub fn to_text(&self) -> String {
        match self {
            Self::Text(s) => s.clone(),
            Self::Texts(v) => v.join("\n"),
            Self::Json(v) => v.to_string(),
            Self::Binary(b) => String::from_utf8_lossy(b).to_string(),
            Self::Stream => "[STREAM]".to_string(),
            Self::Error(e) => format!("Error: {}", e),
            Self::Empty => String::new(),
        }
    }
}

// ============================================================================
// 辅助类型
// ============================================================================

/// 工具使用记录
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolUsage {
    /// 工具名称
    pub name: String,
    /// 工具输入
    pub input: serde_json::Value,
    /// 工具输出
    pub output: Option<serde_json::Value>,
    /// 是否成功
    pub success: bool,
    /// 错误信息
    pub error: Option<String>,
    /// 执行时间 (毫秒)
    pub duration_ms: u64,
}

impl ToolUsage {
    /// 创建成功的工具使用记录
    pub fn success(
        name: impl Into<String>,
        input: serde_json::Value,
        output: serde_json::Value,
        duration_ms: u64,
    ) -> Self {
        Self {
            name: name.into(),
            input,
            output: Some(output),
            success: true,
            error: None,
            duration_ms,
        }
    }

    /// 创建失败的工具使用记录
    pub fn failure(
        name: impl Into<String>,
        input: serde_json::Value,
        error: impl Into<String>,
        duration_ms: u64,
    ) -> Self {
        Self {
            name: name.into(),
            input,
            output: None,
            success: false,
            error: Some(error.into()),
            duration_ms,
        }
    }
}

/// 推理步骤
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReasoningStep {
    /// 步骤类型
    pub step_type: ReasoningStepType,
    /// 步骤内容
    pub content: String,
    /// 步骤序号
    pub step_number: usize,
    /// 时间戳
    pub timestamp_ms: u64,
}

impl ReasoningStep {
    /// 创建新的推理步骤
    pub fn new(step_type: ReasoningStepType, content: impl Into<String>, step_number: usize) -> Self {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;

        Self {
            step_type,
            content: content.into(),
            step_number,
            timestamp_ms: now,
        }
    }
}

/// 推理步骤类型
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ReasoningStepType {
    /// 思考
    Thought,
    /// 行动
    Action,
    /// 观察
    Observation,
    /// 反思
    Reflection,
    /// 决策
    Decision,
    /// 最终答案
    FinalAnswer,
    /// 自定义
    Custom(String),
}

/// Token 使用统计
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TokenUsage {
    /// 提示词 tokens
    pub prompt_tokens: u32,
    /// 完成 tokens
    pub completion_tokens: u32,
    /// 总 tokens
    pub total_tokens: u32,
}

impl TokenUsage {
    pub fn new(prompt_tokens: u32, completion_tokens: u32) -> Self {
        let total_tokens = prompt_tokens + completion_tokens;
        Self {
            prompt_tokens,
            completion_tokens,
            total_tokens,
        }
    }
}

// ============================================================================
// LLM 相关类型
// ============================================================================

/// LLM 聊天完成请求
#[derive(Debug, Clone)]
pub struct ChatCompletionRequest {
    /// Messages for the chat completion
    pub messages: Vec<ChatMessage>,
    /// Model to use
    pub model: Option<String>,
    /// Tool definitions (if tools are available)
    pub tools: Option<Vec<ToolDefinition>>,
    /// Temperature
    pub temperature: Option<f32>,
    /// Max tokens
    pub max_tokens: Option<u32>,
}

/// 聊天消息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    /// Role: system, user, assistant, tool
    pub role: String,
    /// Content (text or structured)
    pub content: Option<String>,
    /// Tool call ID (for tool responses)
    pub tool_call_id: Option<String>,
    /// Tool calls (for assistant messages with tools)
    pub tool_calls: Option<Vec<ToolCall>>,
}

/// LLM 工具调用
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    /// Tool call ID
    pub id: String,
    /// Tool name
    pub name: String,
    /// Tool arguments (as JSON string or Value)
    pub arguments: serde_json::Value,
}

/// LLM 工具定义
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDefinition {
    /// Tool name
    pub name: String,
    /// Tool description
    pub description: String,
    /// Tool parameters (JSON Schema)
    pub parameters: serde_json::Value,
}

/// LLM 聊天完成响应
#[derive(Debug, Clone)]
pub struct ChatCompletionResponse {
    /// Response content
    pub content: Option<String>,
    /// Tool calls from the LLM
    pub tool_calls: Option<Vec<ToolCall>>,
    /// Usage statistics
    pub usage: Option<TokenUsage>,
}

/// LLM Provider trait - 定义 LLM 提供商接口
///
/// 这是一个核心抽象，定义了所有 LLM 提供商必须实现的最小接口。
///
/// # 示例
///
/// ```rust,ignore
/// use mofa_kernel::agent::types::{LLMProvider, ChatCompletionRequest, ChatCompletionResponse};
///
/// struct MyLLMProvider;
///
/// #[async_trait]
/// impl LLMProvider for MyLLMProvider {
///     fn name(&self) -> &str { "my-llm" }
///
///     async fn chat(&self, request: ChatCompletionRequest) -> AgentResult<ChatCompletionResponse> {
///         // 实现 LLM 调用逻辑
///     }
/// }
/// ```
#[async_trait]
pub trait LLMProvider: Send + Sync {
    /// Get provider name
    fn name(&self) -> &str;

    /// Complete a chat request
    async fn chat(&self, request: ChatCompletionRequest) -> super::error::AgentResult<ChatCompletionResponse>;
}

// ============================================================================
// 中断处理
// ============================================================================

/// 中断处理结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum InterruptResult {
    /// 中断已确认，继续执行
    Acknowledged,
    /// 中断导致暂停
    Paused,
    /// 已中断（带部分结果）
    Interrupted {
        /// 部分结果
        partial_result: Option<String>,
    },
    /// 中断导致任务终止
    TaskTerminated {
        /// 部分结果
        partial_result: Option<AgentOutput>,
    },
    /// 中断被忽略（Agent 在关键区段）
    Ignored,
}

// ============================================================================
// 输入输出类型
// ============================================================================

/// 支持的输入类型
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum InputType {
    Text,
    Image,
    Audio,
    Video,
    Structured(String),
    Binary,
}

/// 支持的输出类型
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum OutputType {
    Text,
    Json,
    StructuredJson,
    Stream,
    Binary,
    Multimodal,
}

// ============================================================================
// 辅助函数
// ============================================================================

fn base64_encode(data: &[u8]) -> String {
    const CHARS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut result = Vec::new();

    for chunk in data.chunks(3) {
        let (n, _pad) = match chunk.len() {
            1 => (((chunk[0] as u32) << 16), 2),
            2 => (((chunk[0] as u32) << 16) | ((chunk[1] as u32) << 8), 1),
            _ => (((chunk[0] as u32) << 16) | ((chunk[1] as u32) << 8) | (chunk[2] as u32), 0),
        };

        result.push(CHARS[((n >> 18) & 0x3F) as usize]);
        result.push(CHARS[((n >> 12) & 0x3F) as usize]);

        if chunk.len() > 1 {
            result.push(CHARS[((n >> 6) & 0x3F) as usize]);
        } else {
            result.push(b'=');
        }

        if chunk.len() > 2 {
            result.push(CHARS[(n & 0x3F) as usize]);
        } else {
            result.push(b'=');
        }
    }

    String::from_utf8(result).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_agent_state_transitions() {
        let state = AgentState::Created;
        assert!(state.can_transition_to(&AgentState::Initializing));
        assert!(!state.can_transition_to(&AgentState::Executing));
    }

    #[test]
    fn test_agent_input_text() {
        let input = AgentInput::text("Hello");
        assert_eq!(input.as_text(), Some("Hello"));
        assert_eq!(input.to_text(), "Hello");
    }

    #[test]
    fn test_agent_output_text() {
        let output = AgentOutput::text("World")
            .with_duration(100)
            .with_metadata("key", serde_json::json!("value"));

        assert_eq!(output.as_text(), Some("World"));
        assert_eq!(output.duration_ms, 100);
        assert!(output.metadata.contains_key("key"));
    }

    #[test]
    fn test_tool_usage() {
        let usage = ToolUsage::success(
            "calculator",
            serde_json::json!({"a": 1, "b": 2}),
            serde_json::json!(3),
            50,
        );
        assert!(usage.success);
        assert_eq!(usage.name, "calculator");
    }
}
