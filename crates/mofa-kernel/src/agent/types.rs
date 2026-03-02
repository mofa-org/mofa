//! Agent 核心类型定义
//! Agent core type definitions
//!
//! 定义统一的 Agent 输入、输出和状态类型
//! Defines unified Agent input, output, and state types

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;
use base64::Engine;

// 导出统一类型模块
// Export unified type modules
pub mod error;
pub mod event;
pub mod global;

pub use error::{ErrorCategory, ErrorContext, GlobalError, GlobalResult};
pub use event::{EventBuilder, GlobalEvent};
pub use event::{execution, lifecycle, message, plugin, state};
// 重新导出常用类型
// Re-export common types
pub use global::{GlobalMessage, MessageContent, MessageMetadata};

// ============================================================================
// Agent 状态
// Agent State
// ============================================================================

/// Agent 状态机
/// Agent state machine
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[non_exhaustive]
pub enum AgentState {
    /// 已创建，未初始化
    /// Created, not initialized
    #[default]
    Created,
    /// 正在初始化
    /// Initializing
    Initializing,
    /// 就绪，可执行
    /// Ready, executable
    Ready,
    /// 运行中
    /// Running
    Running,
    /// 正在执行
    /// Executing
    Executing,
    /// 已暂停
    /// Paused
    Paused,
    /// 已中断
    /// Interrupted
    Interrupted,
    /// 正在关闭
    /// Shutting down
    ShuttingDown,
    /// 已终止/关闭
    /// Terminated/Closed
    Shutdown,
    /// 失败状态
    /// Failed state
    Failed,
    /// 销毁
    /// Destroyed
    Destroyed,
    /// 错误状态 (带消息)
    /// Error state (with message)
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
            AgentState::Running => {
                write!(f, "Running")
            }
            AgentState::Destroyed => {
                write!(f, "Destroyed")
            }
        }
    }
}

impl AgentState {
    /// 转换到目标状态
    /// Transition to target state
    pub fn transition_to(
        &self,
        target: AgentState,
    ) -> Result<AgentState, super::error::AgentError> {
        if self.can_transition_to(&target) {
            Ok(target)
        } else {
            Err(super::error::AgentError::invalid_state_transition(
                self, &target,
            ))
        }
    }

    /// 检查是否可以转换到目标状态
    /// Check if transition to target is possible
    pub fn can_transition_to(&self, target: &AgentState) -> bool {
        use AgentState::*;
        matches!(
            (self, target),
            (Created, Initializing)
                | (Initializing, Ready)
                | (Initializing, Error(_))
                | (Initializing, Failed)
                | (Ready, Executing)
                | (Ready, Running)
                | (Ready, ShuttingDown)
                | (Running, Paused)
                | (Running, Executing)
                | (Running, ShuttingDown)
                | (Running, Error(_))
                | (Running, Failed)
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
    /// Whether it is in active state
    pub fn is_active(&self) -> bool {
        matches!(
            self,
            AgentState::Ready | AgentState::Running | AgentState::Executing
        )
    }

    /// 是否为终止状态
    /// Whether it is in terminal state
    pub fn is_terminal(&self) -> bool {
        matches!(
            self,
            AgentState::Shutdown | AgentState::Failed | AgentState::Error(_)
        )
    }
}

// ============================================================================
// Agent 输入
// Agent Input
// ============================================================================

/// Agent 输入类型
/// Agent input type
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[non_exhaustive]
pub enum AgentInput {
    /// 文本输入
    /// Text input
    Text(String),
    /// 多行文本
    /// Multi-line text
    Texts(Vec<String>),
    /// 结构化 JSON
    /// Structured JSON
    Json(serde_json::Value),
    /// 键值对
    /// Key-value pairs
    Map(HashMap<String, serde_json::Value>),
    /// 二进制数据
    /// Binary data
    Binary(Vec<u8>),
    /// 多模态部分内容
    /// Multimodal content parts
    Multimodal(Vec<serde_json::Value>),
    /// 空输入
    /// Empty input
    #[default]
    Empty,
}

impl AgentInput {
    /// 创建文本输入
    /// Create text input
    pub fn text(s: impl Into<String>) -> Self {
        Self::Text(s.into())
    }

    /// 创建 JSON 输入
    /// Create JSON input
    pub fn json(value: serde_json::Value) -> Self {
        Self::Json(value)
    }

    /// 创建键值对输入
    /// Create map input
    pub fn map(map: HashMap<String, serde_json::Value>) -> Self {
        Self::Map(map)
    }

    /// 获取文本内容
    /// Get text content
    pub fn as_text(&self) -> Option<&str> {
        match self {
            Self::Text(s) => Some(s),
            _ => None,
        }
    }

    /// 转换为文本
    /// Convert to text
    pub fn to_text(&self) -> String {
        match self {
            Self::Text(s) => s.clone(),
            Self::Texts(v) => v.join("\n"),
            Self::Json(v) => v.to_string(),
            Self::Map(m) => serde_json::to_string(m).unwrap_or_default(),
            Self::Binary(b) => String::from_utf8_lossy(b).to_string(),
            Self::Multimodal(_) => "[Multimodal Content]".to_string(),
            Self::Empty => String::new(),
        }
    }

    /// 获取 JSON 内容
    /// Get JSON content
    pub fn as_json(&self) -> Option<&serde_json::Value> {
        match self {
            Self::Json(v) => Some(v),
            _ => None,
        }
    }

    /// 转换为 JSON
    /// Convert to JSON
    pub fn to_json(&self) -> serde_json::Value {
        match self {
            Self::Text(s) => serde_json::Value::String(s.clone()),
            Self::Texts(v) => serde_json::json!(v),
            Self::Json(v) => v.clone(),
            Self::Map(m) => serde_json::to_value(m).unwrap_or_default(),
            Self::Multimodal(parts) => serde_json::json!({ "parts": parts }),
            Self::Binary(b) => {
                serde_json::json!({ "binary": base64::engine::general_purpose::STANDARD.encode(b) })
            }
            Self::Empty => serde_json::Value::Null,
        }
    }

    /// 是否为空
    /// Whether it is empty
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
// Agent Output
// ============================================================================

/// Agent 输出类型
/// Agent output type
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentOutput {
    /// 主输出内容
    /// Main output content
    pub content: OutputContent,
    /// 输出元数据
    /// Output metadata
    pub metadata: HashMap<String, serde_json::Value>,
    /// 使用的工具
    /// Tools used
    pub tools_used: Vec<ToolUsage>,
    /// 推理步骤 (如果有)
    /// Reasoning steps (if any)
    pub reasoning_steps: Vec<ReasoningStep>,
    /// 执行时间 (毫秒)
    /// Execution time (ms)
    pub duration_ms: u64,
    /// Token 使用统计
    /// Token usage statistics
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
    /// Create text output
    pub fn text(s: impl Into<String>) -> Self {
        Self {
            content: OutputContent::Text(s.into()),
            ..Default::default()
        }
    }

    /// 创建 JSON 输出
    /// Create JSON output
    pub fn json(value: serde_json::Value) -> Self {
        Self {
            content: OutputContent::Json(value),
            ..Default::default()
        }
    }

    /// 创建错误输出
    /// Create error output
    pub fn error(message: impl Into<String>) -> Self {
        Self {
            content: OutputContent::Error(message.into()),
            ..Default::default()
        }
    }

    /// 获取文本内容
    /// Get text content
    pub fn as_text(&self) -> Option<&str> {
        match &self.content {
            OutputContent::Text(s) => Some(s),
            _ => None,
        }
    }

    /// 转换为文本
    /// Convert to text
    pub fn to_text(&self) -> String {
        self.content.to_text()
    }

    /// 设置执行时间
    /// Set execution duration
    pub fn with_duration(mut self, duration_ms: u64) -> Self {
        self.duration_ms = duration_ms;
        self
    }

    /// 添加元数据
    /// Add metadata
    pub fn with_metadata(mut self, key: impl Into<String>, value: serde_json::Value) -> Self {
        self.metadata.insert(key.into(), value);
        self
    }

    /// 添加工具使用记录
    /// Add tool usage record
    pub fn with_tool_usage(mut self, usage: ToolUsage) -> Self {
        self.tools_used.push(usage);
        self
    }

    /// 设置所有工具使用记录
    /// Set all tool usage records
    pub fn with_tools_used(mut self, usages: Vec<ToolUsage>) -> Self {
        self.tools_used = usages;
        self
    }

    /// 添加推理步骤
    /// Add reasoning step
    pub fn with_reasoning_step(mut self, step: ReasoningStep) -> Self {
        self.reasoning_steps.push(step);
        self
    }

    /// 设置所有推理步骤
    /// Set all reasoning steps
    pub fn with_reasoning_steps(mut self, steps: Vec<ReasoningStep>) -> Self {
        self.reasoning_steps = steps;
        self
    }

    /// 设置 Token 使用
    /// Set token usage
    pub fn with_token_usage(mut self, usage: TokenUsage) -> Self {
        self.token_usage = Some(usage);
        self
    }

    /// 是否为错误
    /// Whether it is an error
    pub fn is_error(&self) -> bool {
        matches!(self.content, OutputContent::Error(_))
    }
}

/// 输出内容类型
/// Output content type
#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub enum OutputContent {
    /// 文本输出
    /// Text output
    Text(String),
    /// 多行文本
    /// Multi-line text
    Texts(Vec<String>),
    /// JSON 输出
    /// JSON output
    Json(serde_json::Value),
    /// 二进制输出
    /// Binary output
    Binary(Vec<u8>),
    /// 流式输出标记
    /// Streaming output marker
    Stream,
    /// 错误输出
    /// Error output
    Error(String),
    /// 空输出
    /// Empty output
    Empty,
}

impl OutputContent {
    /// 转换为文本
    /// Convert to text
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
// Auxiliary types
// ============================================================================

/// 工具使用记录
/// Tool usage record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolUsage {
    /// 工具名称
    /// Tool name
    pub name: String,
    /// 工具输入
    /// Tool input
    pub input: serde_json::Value,
    /// 工具输出
    /// Tool output
    pub output: Option<serde_json::Value>,
    /// 是否成功
    /// Whether successful
    pub success: bool,
    /// 错误信息
    /// Error message
    pub error: Option<String>,
    /// 执行时间 (毫秒)
    /// Execution duration (ms)
    pub duration_ms: u64,
}

impl ToolUsage {
    /// 创建成功的工具使用记录
    /// Create successful tool usage record
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
    /// Create failed tool usage record
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
/// Reasoning step
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReasoningStep {
    /// 步骤类型
    /// Step type
    pub step_type: ReasoningStepType,
    /// 步骤内容
    /// Step content
    pub content: String,
    /// 步骤序号
    /// Step number
    pub step_number: usize,
    /// 时间戳
    /// Timestamp
    pub timestamp_ms: u64,
}

impl ReasoningStep {
    /// 创建新的推理步骤
    /// Create new reasoning step
    pub fn new(
        step_type: ReasoningStepType,
        content: impl Into<String>,
        step_number: usize,
    ) -> Self {
        let now = crate::utils::now_ms();

        Self {
            step_type,
            content: content.into(),
            step_number,
            timestamp_ms: now,
        }
    }
}

/// 推理步骤类型
/// Reasoning step type
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub enum ReasoningStepType {
    /// 思考
    /// Thought
    Thought,
    /// 行动
    /// Action
    Action,
    /// 观察
    /// Observation
    Observation,
    /// 反思
    /// Reflection
    Reflection,
    /// 决策
    /// Decision
    Decision,
    /// 最终答案
    /// Final answer
    FinalAnswer,
    /// 自定义
    /// Custom
    Custom(String),
}

/// Token 使用统计
/// Token usage statistics
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TokenUsage {
    /// 提示词 tokens
    /// Prompt tokens
    pub prompt_tokens: u32,
    /// 完成 tokens
    /// Completion tokens
    pub completion_tokens: u32,
    /// 总 tokens
    /// Total tokens
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
// LLM related types
// ============================================================================

/// LLM 聊天完成请求
/// LLM chat completion request
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
/// Chat message
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
/// LLM tool call
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
/// LLM tool definition
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
/// LLM chat completion response
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
/// LLM Provider trait - Defines LLM provider interface
///
/// 这是一个核心抽象，定义了所有 LLM 提供商必须实现的最小接口。
/// This is a core abstraction defining the minimum interface for LLM providers.
///
/// # 示例
/// # Example
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
///         // Implement LLM call logic
///     }
/// }
/// ```
#[async_trait]
pub trait LLMProvider: Send + Sync {
    /// Get provider name
    fn name(&self) -> &str;

    /// Complete a chat request
    async fn chat(
        &self,
        request: ChatCompletionRequest,
    ) -> super::error::AgentResult<ChatCompletionResponse>;
}

// ============================================================================
// 中断处理
// Interrupt Handling
// ============================================================================

/// 中断处理结果
/// Interrupt handling result
#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub enum InterruptResult {
    /// 中断已确认，继续执行
    /// Interrupt acknowledged, continue execution
    Acknowledged,
    /// 中断导致暂停
    /// Interrupt leads to pause
    Paused,
    /// 已中断（带部分结果）
    /// Interrupted (with partial results)
    Interrupted {
        /// 部分结果
        /// Partial result
        partial_result: Option<String>,
    },
    /// 中断导致任务终止
    /// Interrupt leads to task termination
    TaskTerminated {
        /// 部分结果
        /// Partial result
        partial_result: Option<AgentOutput>,
    },
    /// 中断被忽略（Agent 在关键区段）
    /// Interrupt ignored (Agent in critical section)
    Ignored,
}

// ============================================================================
// 输入输出类型
// Input and Output types
// ============================================================================

/// 支持的输入类型
/// Supported input types
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[non_exhaustive]
pub enum InputType {
    Text,
    Image,
    Audio,
    Video,
    Structured(String),
    Binary,
}

/// 支持的输出类型
/// Supported output types
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[non_exhaustive]
pub enum OutputType {
    Text,
    Json,
    StructuredJson,
    Stream,
    Binary,
    Multimodal,
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
    fn test_agent_state_running_transitions() {
        // Ready -> Running should be valid
        let ready = AgentState::Ready;
        assert!(ready.can_transition_to(&AgentState::Running));

        // Running -> valid targets
        let running = AgentState::Running;
        assert!(running.can_transition_to(&AgentState::Paused));
        assert!(running.can_transition_to(&AgentState::Executing));
        assert!(running.can_transition_to(&AgentState::ShuttingDown));
        assert!(running.can_transition_to(&AgentState::Error("test".to_string())));
        assert!(running.can_transition_to(&AgentState::Failed));

        // Running -> invalid targets
        assert!(!running.can_transition_to(&AgentState::Created));
        assert!(!running.can_transition_to(&AgentState::Ready));
        assert!(!running.can_transition_to(&AgentState::Initializing));
        assert!(!running.can_transition_to(&AgentState::Running));

        // transition_to should return Ok for valid transitions
        let result = ready.transition_to(AgentState::Running);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), AgentState::Running);

        // transition_to should return Err for invalid transitions
        let result = running.transition_to(AgentState::Created);
        assert!(result.is_err());
    }

    #[test]
    fn test_agent_state_is_active() {
        assert!(AgentState::Ready.is_active());
        assert!(AgentState::Running.is_active());
        assert!(AgentState::Executing.is_active());

        assert!(!AgentState::Created.is_active());
        assert!(!AgentState::Paused.is_active());
        assert!(!AgentState::Shutdown.is_active());
        assert!(!AgentState::Failed.is_active());
        assert!(!AgentState::Error("err".to_string()).is_active());
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
