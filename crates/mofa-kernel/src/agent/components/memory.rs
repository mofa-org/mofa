//! 记忆组件
//! Memory component
//!
//! 定义 Agent 的记忆/状态持久化能力
//! Defines the memory and state persistence capabilities of the Agent

use crate::agent::error::AgentResult;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// 记忆组件 Trait
/// Memory component Trait
///
/// 负责 Agent 的记忆存储和检索
/// Responsible for Agent memory storage and retrieval
///
/// # 示例
/// # Example
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
    /// Store a memory item
    async fn store(&mut self, key: &str, value: MemoryValue) -> AgentResult<()>;

    /// 检索记忆项
    /// Retrieve a memory item
    async fn retrieve(&self, key: &str) -> AgentResult<Option<MemoryValue>>;

    /// 删除记忆项
    /// Remove a memory item
    async fn remove(&mut self, key: &str) -> AgentResult<bool>;

    /// 检查是否存在
    /// Check if item exists
    async fn contains(&self, key: &str) -> AgentResult<bool> {
        Ok(self.retrieve(key).await?.is_some())
    }

    /// 语义搜索
    /// Semantic search
    async fn search(&self, query: &str, limit: usize) -> AgentResult<Vec<MemoryItem>>;

    /// 清空所有记忆
    /// Clear all memories
    async fn clear(&mut self) -> AgentResult<()>;

    /// 获取对话历史
    /// Get chat history
    async fn get_history(&self, session_id: &str) -> AgentResult<Vec<Message>>;

    /// 添加对话消息
    /// Add a chat message
    async fn add_to_history(&mut self, session_id: &str, message: Message) -> AgentResult<()>;

    /// 清空对话历史
    /// Clear chat history
    async fn clear_history(&mut self, session_id: &str) -> AgentResult<()>;

    /// 获取记忆统计
    /// Get memory statistics
    async fn stats(&self) -> AgentResult<MemoryStats> {
        Ok(MemoryStats::default())
    }

    /// 记忆类型名称
    /// Memory type name
    fn memory_type(&self) -> &str {
        "memory"
    }
}

/// 记忆值类型
/// Memory value type
#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub enum MemoryValue {
    /// 文本
    /// Text
    Text(String),
    /// 嵌入向量
    /// Embedding vector
    Embedding(Vec<f32>),
    /// 结构化数据
    /// Structured data
    Structured(serde_json::Value),
    /// 二进制数据
    /// Binary data
    Binary(Vec<u8>),
    /// 带嵌入的文本
    /// Text with embedding
    TextWithEmbedding { text: String, embedding: Vec<f32> },
}

impl MemoryValue {
    /// 创建文本值
    /// Create text value
    pub fn text(s: impl Into<String>) -> Self {
        Self::Text(s.into())
    }

    /// 创建嵌入向量值
    /// Create embedding value
    pub fn embedding(e: Vec<f32>) -> Self {
        Self::Embedding(e)
    }

    /// 创建结构化值
    /// Create structured value
    pub fn structured(v: serde_json::Value) -> Self {
        Self::Structured(v)
    }

    /// 创建带嵌入的文本
    /// Create text with embedding
    pub fn text_with_embedding(text: impl Into<String>, embedding: Vec<f32>) -> Self {
        Self::TextWithEmbedding {
            text: text.into(),
            embedding,
        }
    }

    /// 获取文本内容
    /// Get text content
    pub fn as_text(&self) -> Option<&str> {
        match self {
            Self::Text(s) => Some(s),
            Self::TextWithEmbedding { text, .. } => Some(text),
            _ => None,
        }
    }

    /// 获取嵌入向量
    /// Get embedding vector
    pub fn as_embedding(&self) -> Option<&[f32]> {
        match self {
            Self::Embedding(e) => Some(e),
            Self::TextWithEmbedding { embedding, .. } => Some(embedding),
            _ => None,
        }
    }

    /// 获取结构化数据
    /// Get structured data
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
/// Memory item (Search result)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryItem {
    /// 记忆键
    /// Memory key
    pub key: String,
    /// 记忆值
    /// Memory value
    pub value: MemoryValue,
    /// 相似度分数 (0.0 - 1.0)
    /// Similarity score (0.0 - 1.0)
    pub score: f32,
    /// 元数据
    /// Metadata
    pub metadata: HashMap<String, String>,
    /// 创建时间
    /// Creation time
    pub created_at: u64,
    /// 最后访问时间
    /// Last access time
    pub last_accessed: u64,
}

impl MemoryItem {
    /// 创建新的记忆项
    /// Create a new memory item
    pub fn new(key: impl Into<String>, value: MemoryValue) -> Self {
        let now = crate::utils::now_ms();

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
    /// Set score
    pub fn with_score(mut self, score: f32) -> Self {
        self.score = score.clamp(0.0, 1.0);
        self
    }

    /// 添加元数据
    /// Add metadata
    pub fn with_metadata(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.metadata.insert(key.into(), value.into());
        self
    }
}

/// 对话消息
/// Conversation message
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    /// 消息角色
    /// Message role
    pub role: MessageRole,
    /// 消息内容
    /// Message content
    pub content: String,
    /// 时间戳
    /// Timestamp
    pub timestamp: u64,
    /// 元数据
    /// Metadata
    pub metadata: HashMap<String, serde_json::Value>,
}

impl Message {
    /// 创建新消息
    /// Create new message
    pub fn new(role: MessageRole, content: impl Into<String>) -> Self {
        let now = crate::utils::now_ms();

        Self {
            role,
            content: content.into(),
            timestamp: now,
            metadata: HashMap::new(),
        }
    }

    /// 创建系统消息
    /// Create system message
    pub fn system(content: impl Into<String>) -> Self {
        Self::new(MessageRole::System, content)
    }

    /// 创建用户消息
    /// Create user message
    pub fn user(content: impl Into<String>) -> Self {
        Self::new(MessageRole::User, content)
    }

    /// 创建助手消息
    /// Create assistant message
    pub fn assistant(content: impl Into<String>) -> Self {
        Self::new(MessageRole::Assistant, content)
    }

    /// 创建工具消息
    /// Create tool message
    pub fn tool(tool_name: impl Into<String>, content: impl Into<String>) -> Self {
        let mut msg = Self::new(MessageRole::Tool, content);
        msg.metadata.insert(
            "tool_name".to_string(),
            serde_json::Value::String(tool_name.into()),
        );
        msg
    }

    /// 添加元数据
    /// Add metadata
    pub fn with_metadata(mut self, key: impl Into<String>, value: serde_json::Value) -> Self {
        self.metadata.insert(key.into(), value);
        self
    }
}

/// 消息角色
/// Message role
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub enum MessageRole {
    /// 系统消息
    /// System message
    System,
    /// 用户消息
    /// User message
    User,
    /// 助手消息
    /// Assistant message
    Assistant,
    /// 工具消息
    /// Tool message
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
/// Memory statistics
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MemoryStats {
    /// 总记忆项数
    /// Total memory items
    pub total_items: usize,
    /// 总对话会话数
    /// Total chat sessions
    pub total_sessions: usize,
    /// 总消息数
    /// Total messages
    pub total_messages: usize,
    /// 内存使用 (字节)
    /// Memory usage (bytes)
    pub memory_bytes: usize,
}
