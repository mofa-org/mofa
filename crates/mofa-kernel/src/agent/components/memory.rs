//! 记忆组件
//!
//! 定义 Agent 的记忆/状态持久化能力

use crate::agent::error::AgentResult;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// 记忆组件 Trait
///
/// 负责 Agent 的记忆存储和检索
///
/// # 示例
///
/// ```rust,ignore
/// use mofa_kernel::agent::components::memory::{Memory, MemoryValue, MemoryItem};
///
/// struct InMemoryStorage {
///     data: HashMap<String, MemoryValue>,
/// }
///
/// #[async_trait]
/// impl Memory for InMemoryStorage {
///     async fn store(&mut self, key: &str, value: MemoryValue) -> AgentResult<()> {
///         self.data.insert(key.to_string(), value);
///         Ok(())
///     }
///
///     async fn retrieve(&self, key: &str) -> AgentResult<Option<MemoryValue>> {
///         Ok(self.data.get(key).cloned())
///     }
///
///     // ... other methods
/// }
/// ```
#[async_trait]
pub trait Memory: Send + Sync {
    /// 存储记忆项
    async fn store(&mut self, key: &str, value: MemoryValue) -> AgentResult<()>;

    /// 检索记忆项
    async fn retrieve(&self, key: &str) -> AgentResult<Option<MemoryValue>>;

    /// 删除记忆项
    async fn remove(&mut self, key: &str) -> AgentResult<bool>;

    /// 检查是否存在
    async fn contains(&self, key: &str) -> AgentResult<bool> {
        Ok(self.retrieve(key).await?.is_some())
    }

    /// 语义搜索
    async fn search(&self, query: &str, limit: usize) -> AgentResult<Vec<MemoryItem>>;

    /// 清空所有记忆
    async fn clear(&mut self) -> AgentResult<()>;

    /// 获取对话历史
    async fn get_history(&self, session_id: &str) -> AgentResult<Vec<Message>>;

    /// 添加对话消息
    async fn add_to_history(&mut self, session_id: &str, message: Message) -> AgentResult<()>;

    /// 清空对话历史
    async fn clear_history(&mut self, session_id: &str) -> AgentResult<()>;

    /// 获取记忆统计
    async fn stats(&self) -> AgentResult<MemoryStats> {
        Ok(MemoryStats::default())
    }

    /// 记忆类型名称
    fn memory_type(&self) -> &str {
        "memory"
    }
}

/// 记忆值类型
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MemoryValue {
    /// 文本
    Text(String),
    /// 嵌入向量
    Embedding(Vec<f32>),
    /// 结构化数据
    Structured(serde_json::Value),
    /// 二进制数据
    Binary(Vec<u8>),
    /// 带嵌入的文本
    TextWithEmbedding {
        text: String,
        embedding: Vec<f32>,
    },
}

impl MemoryValue {
    /// 创建文本值
    pub fn text(s: impl Into<String>) -> Self {
        Self::Text(s.into())
    }

    /// 创建嵌入向量值
    pub fn embedding(e: Vec<f32>) -> Self {
        Self::Embedding(e)
    }

    /// 创建结构化值
    pub fn structured(v: serde_json::Value) -> Self {
        Self::Structured(v)
    }

    /// 创建带嵌入的文本
    pub fn text_with_embedding(text: impl Into<String>, embedding: Vec<f32>) -> Self {
        Self::TextWithEmbedding {
            text: text.into(),
            embedding,
        }
    }

    /// 获取文本内容
    pub fn as_text(&self) -> Option<&str> {
        match self {
            Self::Text(s) => Some(s),
            Self::TextWithEmbedding { text, .. } => Some(text),
            _ => None,
        }
    }

    /// 获取嵌入向量
    pub fn as_embedding(&self) -> Option<&[f32]> {
        match self {
            Self::Embedding(e) => Some(e),
            Self::TextWithEmbedding { embedding, .. } => Some(embedding),
            _ => None,
        }
    }

    /// 获取结构化数据
    pub fn as_structured(&self) -> Option<&serde_json::Value> {
        match self {
            Self::Structured(v) => Some(v),
            _ => None,
        }
    }
}

impl From<String> for MemoryValue {
    fn from(s: String) -> Self {
        Self::Text(s)
    }
}

impl From<&str> for MemoryValue {
    fn from(s: &str) -> Self {
        Self::Text(s.to_string())
    }
}

impl From<serde_json::Value> for MemoryValue {
    fn from(v: serde_json::Value) -> Self {
        Self::Structured(v)
    }
}

/// 记忆项 (搜索结果)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryItem {
    /// 记忆键
    pub key: String,
    /// 记忆值
    pub value: MemoryValue,
    /// 相似度分数 (0.0 - 1.0)
    pub score: f32,
    /// 元数据
    pub metadata: HashMap<String, String>,
    /// 创建时间
    pub created_at: u64,
    /// 最后访问时间
    pub last_accessed: u64,
}

impl MemoryItem {
    /// 创建新的记忆项
    pub fn new(key: impl Into<String>, value: MemoryValue) -> Self {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;

        Self {
            key: key.into(),
            value,
            score: 1.0,
            metadata: HashMap::new(),
            created_at: now,
            last_accessed: now,
        }
    }

    /// 设置分数
    pub fn with_score(mut self, score: f32) -> Self {
        self.score = score.clamp(0.0, 1.0);
        self
    }

    /// 添加元数据
    pub fn with_metadata(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.metadata.insert(key.into(), value.into());
        self
    }
}

/// 对话消息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    /// 消息角色
    pub role: MessageRole,
    /// 消息内容
    pub content: String,
    /// 时间戳
    pub timestamp: u64,
    /// 元数据
    pub metadata: HashMap<String, serde_json::Value>,
}

impl Message {
    /// 创建新消息
    pub fn new(role: MessageRole, content: impl Into<String>) -> Self {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;

        Self {
            role,
            content: content.into(),
            timestamp: now,
            metadata: HashMap::new(),
        }
    }

    /// 创建系统消息
    pub fn system(content: impl Into<String>) -> Self {
        Self::new(MessageRole::System, content)
    }

    /// 创建用户消息
    pub fn user(content: impl Into<String>) -> Self {
        Self::new(MessageRole::User, content)
    }

    /// 创建助手消息
    pub fn assistant(content: impl Into<String>) -> Self {
        Self::new(MessageRole::Assistant, content)
    }

    /// 创建工具消息
    pub fn tool(tool_name: impl Into<String>, content: impl Into<String>) -> Self {
        let mut msg = Self::new(MessageRole::Tool, content);
        msg.metadata.insert(
            "tool_name".to_string(),
            serde_json::Value::String(tool_name.into()),
        );
        msg
    }

    /// 添加元数据
    pub fn with_metadata(mut self, key: impl Into<String>, value: serde_json::Value) -> Self {
        self.metadata.insert(key.into(), value);
        self
    }
}

/// 消息角色
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum MessageRole {
    /// 系统消息
    System,
    /// 用户消息
    User,
    /// 助手消息
    Assistant,
    /// 工具消息
    Tool,
}

impl std::fmt::Display for MessageRole {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::System => write!(f, "system"),
            Self::User => write!(f, "user"),
            Self::Assistant => write!(f, "assistant"),
            Self::Tool => write!(f, "tool"),
        }
    }
}

/// 记忆统计
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MemoryStats {
    /// 总记忆项数
    pub total_items: usize,
    /// 总对话会话数
    pub total_sessions: usize,
    /// 总消息数
    pub total_messages: usize,
    /// 内存使用 (字节)
    pub memory_bytes: usize,
}

// ============================================================================
// 内存实现
// ============================================================================

/// 简单内存存储
pub struct InMemoryStorage {
    data: HashMap<String, MemoryItem>,
    history: HashMap<String, Vec<Message>>,
}

impl InMemoryStorage {
    /// 创建新的内存存储
    pub fn new() -> Self {
        Self {
            data: HashMap::new(),
            history: HashMap::new(),
        }
    }
}

impl Default for InMemoryStorage {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Memory for InMemoryStorage {
    async fn store(&mut self, key: &str, value: MemoryValue) -> AgentResult<()> {
        let item = MemoryItem::new(key, value);
        self.data.insert(key.to_string(), item);
        Ok(())
    }

    async fn retrieve(&self, key: &str) -> AgentResult<Option<MemoryValue>> {
        Ok(self.data.get(key).map(|item| item.value.clone()))
    }

    async fn remove(&mut self, key: &str) -> AgentResult<bool> {
        Ok(self.data.remove(key).is_some())
    }

    async fn search(&self, query: &str, limit: usize) -> AgentResult<Vec<MemoryItem>> {
        // 简单的关键词匹配搜索
        let query_lower = query.to_lowercase();
        let mut results: Vec<MemoryItem> = self
            .data
            .values()
            .filter(|item| {
                if let Some(text) = item.value.as_text() {
                    text.to_lowercase().contains(&query_lower)
                } else {
                    false
                }
            })
            .cloned()
            .collect();

        results.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
        results.truncate(limit);
        Ok(results)
    }

    async fn clear(&mut self) -> AgentResult<()> {
        self.data.clear();
        Ok(())
    }

    async fn get_history(&self, session_id: &str) -> AgentResult<Vec<Message>> {
        Ok(self.history.get(session_id).cloned().unwrap_or_default())
    }

    async fn add_to_history(&mut self, session_id: &str, message: Message) -> AgentResult<()> {
        self.history
            .entry(session_id.to_string())
            .or_default()
            .push(message);
        Ok(())
    }

    async fn clear_history(&mut self, session_id: &str) -> AgentResult<()> {
        self.history.remove(session_id);
        Ok(())
    }

    async fn stats(&self) -> AgentResult<MemoryStats> {
        let total_messages: usize = self.history.values().map(|v| v.len()).sum();
        Ok(MemoryStats {
            total_items: self.data.len(),
            total_sessions: self.history.len(),
            total_messages,
            memory_bytes: 0, // 简化，不计算实际内存
        })
    }

    fn memory_type(&self) -> &str {
        "in-memory"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_in_memory_storage() {
        let mut storage = InMemoryStorage::new();

        // 存储和检索
        storage.store("key1", MemoryValue::text("value1")).await.unwrap();
        let value = storage.retrieve("key1").await.unwrap();
        assert!(value.is_some());
        assert_eq!(value.unwrap().as_text(), Some("value1"));

        // 删除
        let removed = storage.remove("key1").await.unwrap();
        assert!(removed);
        assert!(storage.retrieve("key1").await.unwrap().is_none());
    }

    #[tokio::test]
    async fn test_conversation_history() {
        let mut storage = InMemoryStorage::new();
        let session = "session-1";

        storage.add_to_history(session, Message::user("Hello")).await.unwrap();
        storage.add_to_history(session, Message::assistant("Hi there!")).await.unwrap();

        let history = storage.get_history(session).await.unwrap();
        assert_eq!(history.len(), 2);
        assert_eq!(history[0].role, MessageRole::User);
        assert_eq!(history[1].role, MessageRole::Assistant);
    }

    #[tokio::test]
    async fn test_search() {
        let mut storage = InMemoryStorage::new();

        storage.store("doc1", MemoryValue::text("Hello world")).await.unwrap();
        storage.store("doc2", MemoryValue::text("Goodbye world")).await.unwrap();
        storage.store("doc3", MemoryValue::text("Hello there")).await.unwrap();

        let results = storage.search("Hello", 10).await.unwrap();
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn test_message_creation() {
        let msg = Message::user("Hello");
        assert_eq!(msg.role, MessageRole::User);
        assert_eq!(msg.content, "Hello");

        let tool_msg = Message::tool("calculator", "Result: 42");
        assert_eq!(tool_msg.role, MessageRole::Tool);
        assert!(tool_msg.metadata.contains_key("tool_name"));
    }
}
