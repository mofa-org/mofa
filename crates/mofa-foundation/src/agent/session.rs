//! Session management for conversation persistence
//!
//! Supports multiple storage backends through a trait abstraction

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tokio::fs;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::sync::RwLock;

use mofa_kernel::agent::error::{AgentError, AgentResult};

// ============================================================================
// Session 数据类型
// Session Data Types
// ============================================================================

/// Session message
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionMessage {
    pub role: String,
    pub content: String,
    #[serde(default = "Utc::now")]
    pub timestamp: DateTime<Utc>,
}

impl SessionMessage {
    /// Create a new session message
    pub fn new(role: impl Into<String>, content: impl Into<String>) -> Self {
        Self {
            role: role.into(),
            content: content.into(),
            timestamp: Utc::now(),
        }
    }
}

/// Conversation session
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub key: String,
    pub messages: Vec<SessionMessage>,
    #[serde(default = "Utc::now")]
    pub created_at: DateTime<Utc>,
    #[serde(default = "Utc::now")]
    pub updated_at: DateTime<Utc>,
    #[serde(default)]
    pub metadata: HashMap<String, Value>,
}

impl Session {
    /// Create a new session
    pub fn new(key: impl Into<String>) -> Self {
        let key = key.into();
        Self {
            key,
            messages: Vec::new(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
            metadata: HashMap::new(),
        }
    }

    /// Add a message to the session
    pub fn add_message(&mut self, role: impl Into<String>, content: impl Into<String>) {
        let msg = SessionMessage::new(role, content);
        self.messages.push(msg);
        self.updated_at = Utc::now();
    }

    /// Get message history (limited to recent messages)
    pub fn get_history(&self, max_messages: usize) -> Vec<SessionMessage> {
        if self.messages.len() > max_messages {
            self.messages[self.messages.len() - max_messages..].to_vec()
        } else {
            self.messages.clone()
        }
    }

    /// Clear all messages
    pub fn clear(&mut self) {
        self.messages.clear();
        self.updated_at = Utc::now();
    }

    /// Get the number of messages
    pub fn len(&self) -> usize {
        self.messages.len()
    }

    /// Check if empty
    pub fn is_empty(&self) -> bool {
        self.messages.is_empty()
    }
}

// ============================================================================
// SessionStorage trait - 多种后端支持
// SessionStorage trait - Multiple backend support
// ============================================================================

/// Session storage backend trait
///
/// This trait provides an abstraction for session storage that can be
/// implemented by different backends (file-based, in-memory, database, etc.).
///
/// Note: A kernel-level storage abstraction exists in mofa_kernel::storage,
/// but this trait is foundation-specific as it works with the concrete Session type.
#[async_trait]
pub trait SessionStorage: Send + Sync {
    /// Load a session by key
    async fn load(&self, key: &str) -> AgentResult<Option<Session>>;

    /// Save a session
    async fn save(&self, session: &Session) -> AgentResult<()>;

    /// Delete a session
    async fn delete(&self, key: &str) -> AgentResult<bool>;

    /// List all session keys
    async fn list(&self) -> AgentResult<Vec<String>>;
}

// ============================================================================
// JSONL 文件存储实现
// JSONL file storage implementation
// ============================================================================

/// JSONL file-based session storage
pub struct JsonlSessionStorage {
    sessions_dir: PathBuf,
}

impl JsonlSessionStorage {
    /// Create a new JSONL session storage
    pub async fn new(workspace: impl AsRef<Path>) -> AgentResult<Self> {
        let sessions_dir = workspace.as_ref().join("sessions");
        fs::create_dir_all(&sessions_dir).await.map_err(|e| {
            AgentError::IoError(format!("Failed to create sessions directory: {}", e))
        })?;

        Ok(Self { sessions_dir })
    }

    /// Get the file path for a session
    fn session_file(&self, key: &str) -> PathBuf {
        let safe_key = key.replace(
            |c: char| !c.is_alphanumeric() && c != '-' && c != ':' && c != '_',
            "_",
        );
        self.sessions_dir.join(format!("{}.jsonl", safe_key))
    }

    /// Load a session from file
    async fn load_session(&self, key: &str) -> AgentResult<Option<Session>> {
        let session_file = self.session_file(key);
        if !session_file.exists() {
            return Ok(None);
        }

        let content = fs::read_to_string(&session_file)
            .await
            .map_err(|e| AgentError::IoError(format!("Failed to read session file: {}", e)))?;

        let mut lines = content.lines();
        let header = lines
            .next()
            .ok_or_else(|| AgentError::SerializationError("Empty session file".to_string()))?;

        // Parse header for metadata
        let header_data: Value = serde_json::from_str(header).map_err(|e| {
            AgentError::SerializationError(format!("Failed to parse session header: {}", e))
        })?;

        let key = header_data
            .get("key")
            .and_then(|v| v.as_str())
            .unwrap_or(key)
            .to_string();

        let created_at = header_data
            .get("created_at")
            .and_then(|v| v.as_str())
            .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
            .map(|dt| dt.with_timezone(&Utc))
            .unwrap_or_else(Utc::now);

        let metadata = header_data
            .get("metadata")
            .and_then(|v| serde_json::from_value::<HashMap<String, Value>>(v.clone()).ok())
            .unwrap_or_default();

        let mut messages = Vec::new();
        for line in lines {
            if let Ok(msg) = serde_json::from_str::<SessionMessage>(line) {
                messages.push(msg);
            }
        }

        Ok(Some(Session {
            key,
            messages,
            created_at,
            updated_at: Utc::now(),
            metadata,
        }))
    }
}

#[async_trait]
impl SessionStorage for JsonlSessionStorage {
    async fn load(&self, key: &str) -> AgentResult<Option<Session>> {
        self.load_session(key).await
    }

    async fn save(&self, session: &Session) -> AgentResult<()> {
        let session_file = self.session_file(&session.key);

        // Ensure directory exists
        if let Some(parent) = session_file.parent() {
            fs::create_dir_all(parent).await.map_err(|e| {
                AgentError::IoError(format!("Failed to create sessions directory: {}", e))
            })?;
        }

        let mut lines = vec![
            serde_json::to_string(&serde_json::json!({
                "key": session.key,
                "created_at": session.created_at.to_rfc3339(),
                "updated_at": session.updated_at.to_rfc3339(),
                "metadata": session.metadata,
            }))
            .map_err(|e| {
                AgentError::SerializationError(format!("Failed to serialize session: {}", e))
            })?,
        ];

        for msg in &session.messages {
            lines.push(serde_json::to_string(msg).map_err(|e| {
                AgentError::SerializationError(format!("Failed to serialize message: {}", e))
            })?);
        }

        fs::write(&session_file, lines.join("\n"))
            .await
            .map_err(|e| AgentError::IoError(format!("Failed to write session file: {}", e)))?;

        Ok(())
    }

    async fn delete(&self, key: &str) -> AgentResult<bool> {
        let session_file = self.session_file(key);
        if session_file.exists() {
            fs::remove_file(&session_file).await.map_err(|e| {
                AgentError::IoError(format!("Failed to remove session file: {}", e))
            })?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    async fn list(&self) -> AgentResult<Vec<String>> {
        let mut entries = fs::read_dir(&self.sessions_dir).await.map_err(|e| {
            AgentError::IoError(format!("Failed to read sessions directory: {}", e))
        })?;

        let mut keys = Vec::new();
        while let Some(entry) = entries
            .next_entry()
            .await
            .map_err(|e| AgentError::IoError(format!("Failed to read entry: {}", e)))?
        {
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) != Some("jsonl") {
                continue;
            }

            if let Ok(file) = fs::File::open(&path).await {
                let mut reader = BufReader::new(file);
                let mut header = String::new();
                if reader.read_line(&mut header).await.is_ok() {
                    let header = header.trim_end();
                    if let Ok(header_data) = serde_json::from_str::<Value>(header)
                        && let Some(key) = header_data.get("key").and_then(|v| v.as_str())
                    {
                        keys.push(key.to_string());
                        continue;
                    }
                }
            }

            if let Some(name) = path.file_stem().and_then(|s| s.to_str()) {
                keys.push(name.to_string());
            }
        }

        Ok(keys)
    }
}

// ============================================================================
// 内存存储实现（用于测试）
// Memory storage implementation (for testing)
// ============================================================================

/// In-memory session storage (for testing)
pub struct MemorySessionStorage {
    sessions: RwLock<HashMap<String, Session>>,
}

impl MemorySessionStorage {
    pub fn new() -> Self {
        Self {
            sessions: RwLock::new(HashMap::new()),
        }
    }
}

impl Default for MemorySessionStorage {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl SessionStorage for MemorySessionStorage {
    async fn load(&self, key: &str) -> AgentResult<Option<Session>> {
        let sessions = self.sessions.read().await;
        Ok(sessions.get(key).cloned())
    }

    async fn save(&self, session: &Session) -> AgentResult<()> {
        let mut sessions = self.sessions.write().await;
        sessions.insert(session.key.clone(), session.clone());
        Ok(())
    }

    async fn delete(&self, key: &str) -> AgentResult<bool> {
        let mut sessions = self.sessions.write().await;
        Ok(sessions.remove(key).is_some())
    }

    async fn list(&self) -> AgentResult<Vec<String>> {
        let sessions = self.sessions.read().await;
        Ok(sessions.keys().cloned().collect())
    }
}

// ============================================================================
// SessionManager - 统一会话管理接口
// SessionManager - Unified session management interface
// ============================================================================

/// Session manager with pluggable storage backend
pub struct SessionManager {
    storage: Box<dyn SessionStorage>,
    cache: RwLock<HashMap<String, Session>>,
}

impl SessionManager {
    /// Create with JSONL file storage
    pub async fn with_jsonl(workspace: impl AsRef<Path>) -> AgentResult<Self> {
        let storage = JsonlSessionStorage::new(workspace).await?;
        Ok(Self {
            storage: Box::new(storage),
            cache: RwLock::new(HashMap::new()),
        })
    }

    /// Create with custom storage backend
    pub fn with_storage(storage: Box<dyn SessionStorage>) -> Self {
        Self {
            storage,
            cache: RwLock::new(HashMap::new()),
        }
    }

    /// Get a session by key without creating a new one
    pub async fn get(&self, key: &str) -> AgentResult<Option<Session>> {
        {
            let cache = self.cache.read().await;
            if let Some(session) = cache.get(key) {
                return Ok(Some(session.clone()));
            }
        }

        self.storage.load(key).await
    }

    /// Get or create a session
    pub async fn get_or_create(&self, key: &str) -> Session {
        // Try cache first
        {
            let cache = self.cache.read().await;
            if let Some(session) = cache.get(key) {
                return session.clone();
            }
        }

        // Try storage
        if let Ok(Some(session)) = self.storage.load(key).await {
            let mut cache = self.cache.write().await;
            cache.insert(key.to_string(), session.clone());
            return session;
        }

        // Create new session
        let session = Session::new(key);
        let mut cache = self.cache.write().await;
        cache.insert(key.to_string(), session.clone());
        session
    }

    /// Save a session
    pub async fn save(&self, session: &Session) -> AgentResult<()> {
        self.storage.save(session).await?;
        let mut cache = self.cache.write().await;
        cache.insert(session.key.clone(), session.clone());
        Ok(())
    }

    /// Delete a session
    pub async fn delete(&self, key: &str) -> AgentResult<bool> {
        let result = self.storage.delete(key).await?;
        let mut cache = self.cache.write().await;
        cache.remove(key);
        Ok(result)
    }

    /// List all session keys
    pub async fn list(&self) -> AgentResult<Vec<String>> {
        self.storage.list().await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_session_creation() {
        let session = Session::new("test:key");
        assert_eq!(session.key, "test:key");
        assert!(session.is_empty());
    }

    #[tokio::test]
    async fn test_session_add_message() {
        let mut session = Session::new("test:key");
        session.add_message("user", "Hello");
        session.add_message("assistant", "Hi there!");

        assert_eq!(session.len(), 2);
        let history = session.get_history(10);
        assert_eq!(history.len(), 2);
        assert_eq!(history[0].role, "user");
    }

    #[tokio::test]
    async fn test_memory_storage() {
        let storage = MemorySessionStorage::new();
        let session = Session::new("test:memory");

        storage.save(&session).await.unwrap();
        let loaded = storage.load("test:memory").await.unwrap();
        assert!(loaded.is_some());
        assert_eq!(loaded.unwrap().key, "test:memory");
    }

    #[tokio::test]
    async fn test_jsonl_storage() {
        let temp_dir = TempDir::new().unwrap();
        let storage = JsonlSessionStorage::new(temp_dir.path()).await.unwrap();

        let mut session = Session::new("test:jsonl");
        session.add_message("user", "Hello");

        storage.save(&session).await.unwrap();

        let loaded = storage.load("test:jsonl").await.unwrap();
        assert!(loaded.is_some());
        let loaded_session = loaded.unwrap();
        assert_eq!(loaded_session.key, "test:jsonl");
        assert_eq!(loaded_session.len(), 1);
    }

    #[tokio::test]
    async fn test_session_manager() {
        let temp_dir = TempDir::new().unwrap();
        let manager = SessionManager::with_jsonl(temp_dir.path()).await.unwrap();

        let session = manager.get_or_create("test:manager").await;
        assert_eq!(session.key, "test:manager");

        manager.save(&session).await.unwrap();

        // Reload
        let loaded = manager.get_or_create("test:manager").await;
        assert_eq!(loaded.key, "test:manager");
    }

    #[tokio::test]
    async fn test_session_get_returns_none_for_missing() {
        let temp_dir = TempDir::new().unwrap();
        let manager = SessionManager::with_jsonl(temp_dir.path()).await.unwrap();

        let result = manager.get("nonexistent").await.unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_session_get_returns_some_for_existing() {
        let temp_dir = TempDir::new().unwrap();
        let manager = SessionManager::with_jsonl(temp_dir.path()).await.unwrap();

        let mut session = Session::new("exists");
        session.add_message("user", "hello");
        manager.save(&session).await.unwrap();

        let result = manager.get("exists").await.unwrap();
        assert!(result.is_some());
        assert_eq!(result.unwrap().len(), 1);
    }

    #[tokio::test]
    async fn test_session_get_does_not_create() {
        let temp_dir = TempDir::new().unwrap();
        let manager = SessionManager::with_jsonl(temp_dir.path()).await.unwrap();

        let _ = manager.get("phantom").await.unwrap();
        let keys = manager.list().await.unwrap();
        assert!(!keys.contains(&"phantom".to_string()));
    }

    #[tokio::test]
    async fn test_session_storage_path_no_double_nesting() {
        let temp_dir = TempDir::new().unwrap();
        let manager = SessionManager::with_jsonl(temp_dir.path()).await.unwrap();

        let mut session = Session::new("nesting-test");
        session.add_message("user", "hello");
        manager.save(&session).await.unwrap();

        assert!(
            temp_dir
                .path()
                .join("sessions")
                .join("nesting-test.jsonl")
                .exists()
        );
        assert!(!temp_dir.path().join("sessions").join("sessions").exists());
    }

    #[tokio::test]
    async fn test_session_list_preserves_underscore_keys() {
        let temp_dir = TempDir::new().unwrap();
        let manager = SessionManager::with_jsonl(temp_dir.path()).await.unwrap();

        let mut session = Session::new("team_alpha");
        session.add_message("user", "hello");
        manager.save(&session).await.unwrap();

        let keys = manager.list().await.unwrap();
        assert!(keys.contains(&"team_alpha".to_string()));
        assert!(!keys.contains(&"team:alpha".to_string()));
    }

    #[tokio::test]
    async fn test_session_list_prefers_header_key_without_loading_whole_file() {
        let temp_dir = TempDir::new().unwrap();
        let manager = SessionManager::with_jsonl(temp_dir.path()).await.unwrap();

        let sessions_dir = temp_dir.path().join("sessions");
        tokio::fs::write(
            sessions_dir.join("alias.jsonl"),
            "{\"key\":\"canonical:key\"}\n{\"role\":\"user\",\"content\":\"hello\"}\n",
        )
        .await
        .unwrap();

        let keys = manager.list().await.unwrap();
        assert!(keys.contains(&"canonical:key".to_string()));
        assert!(!keys.contains(&"alias".to_string()));
    }
}
