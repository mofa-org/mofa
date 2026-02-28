//! LLM 提供者抽象
//! LLM provider abstraction
//!
//! 定义与 LLM 交互的抽象接口。
//! Defines the abstract interface for interacting with LLMs.

use mofa_kernel::agent::types::error::{GlobalError, GlobalResult};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;

// =============================================================================
// LLM 提供者 Trait
// LLM Provider Trait
// =============================================================================

/// LLM 提供者 Trait
/// LLM Provider Trait
///
/// 允许秘书 Agent 接入不同的 LLM 服务进行智能处理。
/// Allows the Secretary Agent to access different LLM services for intelligent processing.
///
/// # 示例
/// # Example
///
/// ```rust,ignore
/// struct OpenAIProvider {
///     api_key: String,
///     model: String,
/// }
///
/// #[async_trait]
/// impl LLMProvider for OpenAIProvider {
///     fn name(&self) -> &str {
///         "openai"
///     }
///
///     async fn chat(&self, messages: Vec<ChatMessage>) -> GlobalResult<String> {
///         // 调用 OpenAI API
///         // Call OpenAI API
///         let response = call_openai(&self.api_key, &self.model, messages).await?;
///         Ok(response)
///     }
/// }
/// ```
#[async_trait]
pub trait LLMProvider: Send + Sync {
    /// 提供者名称
    /// Provider name
    fn name(&self) -> &str;

    /// 发送消息并获取响应
    /// Send messages and get response
    async fn chat(&self, messages: Vec<ChatMessage>) -> GlobalResult<String>;

    /// 流式响应（可选实现）
    /// Streaming response (optional implementation)
    async fn chat_stream(
        &self,
        messages: Vec<ChatMessage>,
        tx: mpsc::Sender<String>,
    ) -> GlobalResult<()> {
        // 默认实现：一次性返回
        // Default implementation: return all at once
        let response = self.chat(messages).await?;
        tx.send(response)
            .await
            .map_err(|e| GlobalError::Other(format!("Failed to send: {}", e)))?;
        Ok(())
    }

    /// 获取模型信息（可选实现）
    /// Get model information (optional implementation)
    fn model_info(&self) -> Option<ModelInfo> {
        None
    }
}

// =============================================================================
// 聊天消息
// Chat Message
// =============================================================================

/// 聊天消息
/// Chat message
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    /// 角色: system, user, assistant
    /// Role: system, user, assistant
    pub role: String,
    /// 消息内容
    /// Message content
    pub content: String,
}

impl ChatMessage {
    /// 创建系统消息
    /// Create system message
    pub fn system(content: impl Into<String>) -> Self {
        Self {
            role: "system".to_string(),
            content: content.into(),
        }
    }

    /// 创建用户消息
    /// Create user message
    pub fn user(content: impl Into<String>) -> Self {
        Self {
            role: "user".to_string(),
            content: content.into(),
        }
    }

    /// 创建助手消息
    /// Create assistant message
    pub fn assistant(content: impl Into<String>) -> Self {
        Self {
            role: "assistant".to_string(),
            content: content.into(),
        }
    }
}

// =============================================================================
// 模型信息
// Model Info
// =============================================================================

/// 模型信息
/// Model information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelInfo {
    /// 模型名称
    /// Model name
    pub name: String,
    /// 模型版本
    /// Model version
    pub version: Option<String>,
    /// 上下文窗口大小
    /// Context window size
    pub context_window: Option<usize>,
    /// 最大输出 token 数
    /// Maximum output tokens
    pub max_output_tokens: Option<usize>,
}

// =============================================================================
// JSON 解析辅助函数
// JSON Parsing Helper Functions
// =============================================================================

/// 从 LLM 响应中解析 JSON
/// Parse JSON from LLM response
///
/// 尝试从响应中提取 JSON 并反序列化为指定类型。
/// Attempts to extract JSON from the response and deserialize it into the specified type.
///
/// # 示例
/// # Example
///
/// ```rust,ignore
/// #[derive(Deserialize)]
/// struct TaskAnalysis {
///     title: String,
///     priority: String,
/// }
///
/// let response = llm.chat(messages).await?;
/// let analysis: TaskAnalysis = parse_llm_json(&response)?;
/// ```
pub fn parse_llm_json<T: serde::de::DeserializeOwned>(response: &str) -> GlobalResult<T> {
    let json_str = extract_json_block(response).unwrap_or(response);
    serde_json::from_str(json_str).map_err(|e| GlobalError::Other(format!("JSON parse error: {}", e)))
}

/// 从响应中提取 JSON 块
/// Extract JSON block from response
///
/// 支持从 markdown 代码块或原始 JSON 中提取。
/// Supports extraction from markdown code blocks or raw JSON.
pub fn extract_json_block(text: &str) -> Option<&str> {
    // 查找 ```json ... ```
    // Find ```json ... ```
    if let Some(start) = text.find("```json") {
        let content = &text[start + 7..];
        if let Some(end) = content.find("```") {
            return Some(content[..end].trim());
        }
    }

    // 查找 ``` ... ```（不带语言标识）
    // Find ``` ... ``` (without language identifier)
    if let Some(start) = text.find("```") {
        let content = &text[start + 3..];
        if let Some(end) = content.find("```") {
            let block = content[..end].trim();
            // 检查是否像 JSON
            // Check if it looks like JSON
            if block.starts_with('{') || block.starts_with('[') {
                return Some(block);
            }
        }
    }

    // 尝试直接查找 { ... } 或 [ ... ]
    // Attempt to directly find { ... } or [ ... ]
    if let Some(start) = text.find('{')
        && let Some(end) = text.rfind('}')
        && end > start
    {
        return Some(&text[start..=end]);
    }

    if let Some(start) = text.find('[')
        && let Some(end) = text.rfind(']')
        && end > start
    {
        return Some(&text[start..=end]);
    }

    None
}

// =============================================================================
// 对话历史管理
// Conversation History Management
// =============================================================================

/// 对话历史
/// Conversation history
#[derive(Debug, Clone, Default)]
pub struct ConversationHistory {
    /// 消息列表
    /// Message list
    messages: Vec<ChatMessage>,
    /// 最大消息数量
    /// Maximum message count
    max_messages: Option<usize>,
}

impl ConversationHistory {
    /// 创建新的对话历史
    /// Create new conversation history
    pub fn new() -> Self {
        Self::default()
    }

    /// 设置最大消息数量
    /// Set maximum message count
    pub fn with_max_messages(mut self, max: usize) -> Self {
        self.max_messages = Some(max);
        self
    }

    /// 添加消息
    /// Add message
    pub fn push(&mut self, message: ChatMessage) {
        self.messages.push(message);

        // 如果超过最大数量，移除最早的消息（保留系统消息）
        // If exceeding maximum, remove the earliest message (keep system messages)
        if let Some(max) = self.max_messages {
            while self.messages.len() > max {
                // 找到第一个非系统消息并移除
                // Find the first non-system message and remove it
                if let Some(idx) = self.messages.iter().position(|m| m.role != "system") {
                    self.messages.remove(idx);
                } else {
                    break;
                }
            }
        }
    }

    /// 添加系统消息
    /// Add system message
    pub fn add_system(&mut self, content: impl Into<String>) {
        self.push(ChatMessage::system(content));
    }

    /// 添加用户消息
    /// Add user message
    pub fn add_user(&mut self, content: impl Into<String>) {
        self.push(ChatMessage::user(content));
    }

    /// 添加助手消息
    /// Add assistant message
    pub fn add_assistant(&mut self, content: impl Into<String>) {
        self.push(ChatMessage::assistant(content));
    }

    /// 获取所有消息
    /// Get all messages
    pub fn messages(&self) -> &[ChatMessage] {
        &self.messages
    }

    /// 转换为 Vec
    /// Convert to Vec
    pub fn to_vec(&self) -> Vec<ChatMessage> {
        self.messages.clone()
    }

    /// 清空历史（保留系统消息）
    /// Clear history (except system messages)
    pub fn clear_except_system(&mut self) {
        self.messages.retain(|m| m.role == "system");
    }

    /// 完全清空
    /// Clear completely
    pub fn clear(&mut self) {
        self.messages.clear();
    }

    /// 获取消息数量
    /// Get message count
    pub fn len(&self) -> usize {
        self.messages.len()
    }

    /// 是否为空
    /// Is empty
    pub fn is_empty(&self) -> bool {
        self.messages.is_empty()
    }
}

// =============================================================================
// 测试
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chat_message() {
        let system = ChatMessage::system("You are a helpful assistant.");
        assert_eq!(system.role, "system");

        let user = ChatMessage::user("Hello!");
        assert_eq!(user.role, "user");

        let assistant = ChatMessage::assistant("Hi there!");
        assert_eq!(assistant.role, "assistant");
    }

    #[test]
    fn test_extract_json_block() {
        // 测试 markdown 代码块
        // Test markdown code block
        let text = r#"Here is the result:
```json
{"name": "test", "value": 42}
```
That's all."#;
        let json = extract_json_block(text).unwrap();
        assert_eq!(json, r#"{"name": "test", "value": 42}"#);

        // 测试直接 JSON
        // Test direct JSON
        let text2 = r#"The result is {"name": "test"}"#;
        let json2 = extract_json_block(text2).unwrap();
        assert_eq!(json2, r#"{"name": "test"}"#);
    }

    #[test]
    fn test_conversation_history() {
        let mut history = ConversationHistory::new().with_max_messages(5);

        history.add_system("System prompt");
        history.add_user("Hello");
        history.add_assistant("Hi");
        history.add_user("How are you?");
        history.add_assistant("I'm fine");

        assert_eq!(history.len(), 5);

        // 添加更多消息，应该移除旧消息但保留系统消息
        // Add more messages; should remove old ones but keep system messages
        history.add_user("Another message");
        assert_eq!(history.len(), 5);
        assert_eq!(history.messages()[0].role, "system");
    }
}
