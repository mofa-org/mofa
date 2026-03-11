//! Episodic memory implementation for MoFA agents.
//!
//! Episodic memory stores each conversation turn as a timestamped episode
//! with session context. Unlike the basic `InMemoryStorage`, it provides
//! cross-session retrieval — agents can look back at what happened in
//! previous sessions rather than only the current one.
//!
//! # Architecture
//!
//! This follows the kernel/foundation split: the `Memory` trait is defined
//! in `mofa-kernel`, and this concrete implementation lives in `mofa-foundation`.
//!
//! # Example
//!
//! ```rust,ignore
//! use mofa_foundation::agent::components::episodic_memory::EpisodicMemory;
//! use mofa_kernel::agent::components::memory::{Memory, Message};
//!
//! let mut mem = EpisodicMemory::new();
//!
//! // Store a past conversation
//! mem.add_to_history("session-1", Message::user("what is rust?")).await?;
//! mem.add_to_history("session-1", Message::assistant("Rust is a systems language.")).await?;
//!
//! // Later in a new session, recall recent episodes
//! let recent = mem.get_recent_episodes(5);
//! for ep in recent {
//!     println!("[{}] {}: {}", ep.session_id, ep.message.role, ep.message.content);
//! }
//! ```

use async_trait::async_trait;
use mofa_kernel::agent::components::memory::{
    Memory, MemoryItem, MemoryStats, MemoryValue, Message, MessageRole,
};
use mofa_kernel::agent::error::AgentResult;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

/// A single stored episode — one message within a session, with ordering metadata.
#[derive(Debug, Clone)]
pub struct Episode {
    /// Monotonically increasing ID for global ordering across all sessions.
    pub episode_id: u64,
    /// Session this episode belongs to.
    pub session_id: String,
    /// The message stored in this episode.
    pub message: Message,
}

impl Episode {
    fn new(episode_id: u64, session_id: impl Into<String>, message: Message) -> Self {
        Self {
            episode_id,
            session_id: session_id.into(),
            message,
        }
    }
}

/// Episodic memory that stores conversation turns across multiple sessions.
///
/// Provides cross-session recall: an agent can retrieve recent episodes
/// from any previous session, not just the current one. Search is
/// keyword-based over episode content.
///
/// For vector-similarity search over past interactions, see `SemanticMemory`.
pub struct EpisodicMemory {
    /// session_id → ordered list of episodes for that session
    sessions: HashMap<String, Vec<Episode>>,
    /// all episodes in insertion order (for cross-session recent retrieval)
    all_episodes: Vec<Episode>,
    /// key-value store for named memory items
    kv: HashMap<String, MemoryItem>,
    /// global episode counter
    counter: Arc<AtomicU64>,
}

impl EpisodicMemory {
    /// Create a new empty episodic memory store.
    pub fn new() -> Self {
        Self {
            sessions: HashMap::new(),
            all_episodes: Vec::new(),
            kv: HashMap::new(),
            counter: Arc::new(AtomicU64::new(0)),
        }
    }

    /// Return the N most recent episodes across all sessions, newest last.
    ///
    /// This is the primary cross-session recall API. An agent can call this
    /// at the start of a new session to load context from past conversations.
    pub fn get_recent_episodes(&self, n: usize) -> Vec<&Episode> {
        let total = self.all_episodes.len();
        if n >= total {
            self.all_episodes.iter().collect()
        } else {
            self.all_episodes[total - n..].iter().collect()
        }
    }

    /// Return all episodes for a specific session, in chronological order.
    pub fn get_session_episodes(&self, session_id: &str) -> Vec<&Episode> {
        self.sessions
            .get(session_id)
            .map(|v| v.iter().collect())
            .unwrap_or_default()
    }

    /// Return all known session IDs, sorted alphabetically.
    pub fn session_ids(&self) -> Vec<&str> {
        let mut ids: Vec<&str> = self.sessions.keys().map(|s| s.as_str()).collect();
        ids.sort();
        ids
    }

    /// Total number of episodes stored across all sessions.
    pub fn total_episodes(&self) -> usize {
        self.all_episodes.len()
    }

    fn next_id(&self) -> u64 {
        self.counter.fetch_add(1, Ordering::Relaxed)
    }
}

impl Default for EpisodicMemory {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Memory for EpisodicMemory {
    async fn store(&mut self, key: &str, value: MemoryValue) -> AgentResult<()> {
        let item = MemoryItem::new(key, value);
        self.kv.insert(key.to_string(), item);
        Ok(())
    }

    async fn retrieve(&self, key: &str) -> AgentResult<Option<MemoryValue>> {
        Ok(self.kv.get(key).map(|item| item.value.clone()))
    }

    async fn remove(&mut self, key: &str) -> AgentResult<bool> {
        Ok(self.kv.remove(key).is_some())
    }

    /// Search episodes by keyword across all sessions.
    async fn search(&self, query: &str, limit: usize) -> AgentResult<Vec<MemoryItem>> {
        let query_lower = query.to_lowercase();
        let mut results: Vec<MemoryItem> = self
            .all_episodes
            .iter()
            .filter(|ep| ep.message.content.to_lowercase().contains(&query_lower))
            .map(|ep| {
                let key = format!("{}:{}", ep.session_id, ep.episode_id);
                MemoryItem::new(key, MemoryValue::text(ep.message.content.clone()))
                    .with_metadata("session_id", ep.session_id.clone())
                    .with_metadata("role", ep.message.role.to_string())
                    .with_metadata("episode_id", ep.episode_id.to_string())
            })
            .collect();

        results.truncate(limit);
        Ok(results)
    }

    async fn clear(&mut self) -> AgentResult<()> {
        self.sessions.clear();
        self.all_episodes.clear();
        self.kv.clear();
        Ok(())
    }

    async fn get_history(&self, session_id: &str) -> AgentResult<Vec<Message>> {
        let messages = self
            .sessions
            .get(session_id)
            .map(|episodes| episodes.iter().map(|ep| ep.message.clone()).collect())
            .unwrap_or_default();
        Ok(messages)
    }

    async fn add_to_history(&mut self, session_id: &str, message: Message) -> AgentResult<()> {
        let id = self.next_id();
        let episode = Episode::new(id, session_id, message);

        self.sessions
            .entry(session_id.to_string())
            .or_default()
            .push(episode.clone());

        self.all_episodes.push(episode);
        Ok(())
    }

    async fn clear_history(&mut self, session_id: &str) -> AgentResult<()> {
        self.sessions.remove(session_id);
        self.all_episodes
            .retain(|ep| ep.session_id != session_id);
        Ok(())
    }

    async fn stats(&self) -> AgentResult<MemoryStats> {
        let total_messages = self.all_episodes.len();
        let total_sessions = self.sessions.len();
        Ok(MemoryStats {
            total_items: self.kv.len(),
            total_sessions,
            total_messages,
            memory_bytes: 0,
        })
    }

    fn memory_type(&self) -> &str {
        "episodic"
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use mofa_kernel::agent::components::memory::Message;

    #[tokio::test]
    async fn test_add_and_retrieve_history() {
        let mut mem = EpisodicMemory::new();

        mem.add_to_history("s1", Message::user("hello")).await.unwrap();
        mem.add_to_history("s1", Message::assistant("hi there")).await.unwrap();

        let history = mem.get_history("s1").await.unwrap();
        assert_eq!(history.len(), 2);
        assert_eq!(history[0].content, "hello");
        assert_eq!(history[1].content, "hi there");
    }

    #[tokio::test]
    async fn test_cross_session_episodes() {
        let mut mem = EpisodicMemory::new();

        mem.add_to_history("s1", Message::user("session one message"))
            .await
            .unwrap();
        mem.add_to_history("s2", Message::user("session two message"))
            .await
            .unwrap();
        mem.add_to_history("s3", Message::user("session three message"))
            .await
            .unwrap();

        assert_eq!(mem.total_episodes(), 3);

        let recent = mem.get_recent_episodes(2);
        assert_eq!(recent.len(), 2);
        assert_eq!(recent[0].session_id, "s2");
        assert_eq!(recent[1].session_id, "s3");
    }

    #[tokio::test]
    async fn test_search_across_sessions() {
        let mut mem = EpisodicMemory::new();

        mem.add_to_history("s1", Message::user("I love Rust programming"))
            .await
            .unwrap();
        mem.add_to_history("s2", Message::user("Python is great for data science"))
            .await
            .unwrap();
        mem.add_to_history("s3", Message::assistant("Rust has zero-cost abstractions"))
            .await
            .unwrap();

        let results = mem.search("rust", 10).await.unwrap();
        assert_eq!(results.len(), 2);
    }

    #[tokio::test]
    async fn test_clear_history_for_session() {
        let mut mem = EpisodicMemory::new();

        mem.add_to_history("s1", Message::user("msg1")).await.unwrap();
        mem.add_to_history("s2", Message::user("msg2")).await.unwrap();

        mem.clear_history("s1").await.unwrap();

        let s1_history = mem.get_history("s1").await.unwrap();
        assert!(s1_history.is_empty());

        // s2 should be unaffected
        let s2_history = mem.get_history("s2").await.unwrap();
        assert_eq!(s2_history.len(), 1);

        // cross-session store should also drop s1 episodes
        assert_eq!(mem.total_episodes(), 1);
    }

    #[tokio::test]
    async fn test_kv_store() {
        let mut mem = EpisodicMemory::new();

        mem.store("user_name", MemoryValue::text("Alice")).await.unwrap();
        let val = mem.retrieve("user_name").await.unwrap();
        assert!(val.is_some());
        assert_eq!(val.unwrap().as_text(), Some("Alice"));
    }

    #[tokio::test]
    async fn test_stats() {
        let mut mem = EpisodicMemory::new();

        mem.add_to_history("s1", Message::user("a")).await.unwrap();
        mem.add_to_history("s1", Message::assistant("b")).await.unwrap();
        mem.add_to_history("s2", Message::user("c")).await.unwrap();

        let stats = mem.stats().await.unwrap();
        assert_eq!(stats.total_sessions, 2);
        assert_eq!(stats.total_messages, 3);
    }
}
