//! Semantic memory implementation for MoFA agents.
//!
//! Semantic memory uses vector embeddings to store and retrieve memories
//! by meaning rather than exact keyword match. When an agent stores a memory
//! entry, the text is embedded into a fixed-dimensional vector and indexed in
//! an `InMemoryVectorStore`. At retrieval time, the query is embedded with the
//! same embedder and the most semantically similar memories are returned.
//!
//! Two types are provided:
//!
//! - `HashEmbedder`: a self-contained, API-free embedder based on FNV-1a hashing
//!   over words and character bigrams. Suitable for development, testing, and
//!   scenarios where an external embedding API is not available.
//!
//! - `SemanticMemory`: implements the kernel `Memory` trait, backed by
//!   `InMemoryVectorStore` for similarity search and a `HashMap` for chat history.
//!
//! # Example
//!
//! ```rust,ignore
//! use mofa_foundation::agent::components::semantic_memory::{HashEmbedder, SemanticMemory};
//! use mofa_kernel::agent::components::memory::{Memory, MemoryValue};
//! use std::sync::Arc;
//!
//! let embedder = Arc::new(HashEmbedder::new(128));
//! let mut mem = SemanticMemory::new(embedder);
//!
//! mem.store("note1", MemoryValue::text("Rust is a systems programming language")).await?;
//! mem.store("note2", MemoryValue::text("Python is popular in machine learning")).await?;
//!
//! let results = mem.search("systems language", 3).await?;
//! // "note1" should rank first because its embedding is closest to the query
//! ```

use async_trait::async_trait;
use mofa_kernel::agent::components::memory::{
    Embedder, Memory, MemoryItem, MemoryStats, MemoryValue, Message,
};
use mofa_kernel::agent::error::{AgentError, AgentResult};
use mofa_kernel::rag::{DocumentChunk, SimilarityMetric};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::rag::vector_store::InMemoryVectorStore;
use mofa_kernel::rag::VectorStore;

// ============================================================================
// HashEmbedder
// ============================================================================

/// A deterministic, API-free text embedder based on FNV-1a hashing.
///
/// Combines word-level and character-bigram features into a fixed-dimensional
/// L2-normalised vector. Semantically similar texts (sharing words and
/// character patterns) produce vectors with a higher cosine similarity than
/// unrelated texts.
///
/// This embedder requires no external dependencies or network calls, making it
/// ideal for tests, examples, and offline deployments. For production use you
/// can swap in an API-backed embedder (e.g. OpenAI `text-embedding-3-small`)
/// by implementing the `Embedder` trait.
pub struct HashEmbedder {
    dims: usize,
}

impl HashEmbedder {
    /// Create a new `HashEmbedder` with the given number of dimensions.
    ///
    /// Larger values increase the representational capacity of the embedding
    /// but also increase memory usage. 128 is a good default for small datasets.
    pub fn new(dims: usize) -> Self {
        assert!(dims > 0, "Embedding dimensions must be > 0");
        Self { dims }
    }

    /// Create a `HashEmbedder` with 128 dimensions (recommended default).
    pub fn with_128_dims() -> Self {
        Self::new(128)
    }
}

impl Default for HashEmbedder {
    fn default() -> Self {
        Self::with_128_dims()
    }
}

#[async_trait]
impl Embedder for HashEmbedder {
    async fn embed(&self, text: &str) -> AgentResult<Vec<f32>> {
        let mut embedding = vec![0.0f32; self.dims];

        let text_lower = text.to_lowercase();

        // Word-level features (weight 1.0)
        for word in text_lower.split_whitespace() {
            let h = fnv1a(word.as_bytes());
            embedding[h as usize % self.dims] += 1.0;
        }

        // Character bigram features (weight 0.5) — improves recall for partial matches
        let chars: Vec<u8> = text_lower.bytes().collect();
        for bigram in chars.windows(2) {
            let h = fnv1a(bigram);
            embedding[h as usize % self.dims] += 0.5;
        }

        // L2 normalise so cosine similarity in VectorStore is well-behaved
        let norm: f32 = embedding.iter().map(|x| x * x).sum::<f32>().sqrt();
        if norm > 0.0 {
            for v in &mut embedding {
                *v /= norm;
            }
        }

        Ok(embedding)
    }

    fn dimensions(&self) -> usize {
        self.dims
    }

    fn name(&self) -> &str {
        "hash-embedder"
    }
}

/// FNV-1a 64-bit hash — fast and deterministic, suitable for feature hashing.
fn fnv1a(bytes: &[u8]) -> u64 {
    let mut hash: u64 = 14695981039346656037;
    for &b in bytes {
        hash ^= b as u64;
        hash = hash.wrapping_mul(1099511628211);
    }
    hash
}

// ============================================================================
// SemanticMemory
// ============================================================================

/// Semantic memory backed by an `InMemoryVectorStore` and a pluggable `Embedder`.
///
/// When a memory entry is stored, its text content is embedded and indexed in
/// the vector store. Searching with a natural-language query returns the
/// top-k most semantically similar entries via cosine similarity.
///
/// Chat history is kept in a plain `HashMap` and can be retrieved by session ID.
/// Callers may combine `SemanticMemory` with `EpisodicMemory` for richer recall.
pub struct SemanticMemory {
    store: Arc<RwLock<InMemoryVectorStore>>,
    embedder: Arc<dyn Embedder>,
    /// Maps chunk id → original MemoryItem for retrieval after vector search
    items: HashMap<String, MemoryItem>,
    history: HashMap<String, Vec<Message>>,
}

impl SemanticMemory {
    /// Create a new `SemanticMemory` with the given embedder.
    ///
    /// Uses cosine similarity for vector search (the standard default).
    pub fn new(embedder: Arc<dyn Embedder>) -> Self {
        Self {
            store: Arc::new(RwLock::new(InMemoryVectorStore::new(
                SimilarityMetric::Cosine,
            ))),
            embedder,
            items: HashMap::new(),
            history: HashMap::new(),
        }
    }

    /// Create a `SemanticMemory` using the built-in `HashEmbedder` (128 dims).
    ///
    /// This is the easiest way to get started without configuring an external
    /// embedding API.
    pub fn with_hash_embedder() -> Self {
        Self::new(Arc::new(HashEmbedder::with_128_dims()))
    }
}

#[async_trait]
impl Memory for SemanticMemory {
    /// Store a memory entry by key.
    ///
    /// If the value contains text, it is embedded and indexed in the vector
    /// store so it can be retrieved via `search`. Non-text values (binary,
    /// structured data) are stored in the key-value map only.
    async fn store(&mut self, key: &str, value: MemoryValue) -> AgentResult<()> {
        let item = MemoryItem::new(key, value.clone());

        if let Some(text) = value.as_text() {
            let embedding = self.embedder.embed(text).await?;
            let chunk = DocumentChunk::new(key, text, embedding);
            let mut store = self.store.write().await;
            store
                .upsert(chunk)
                .await
                .map_err(|e| AgentError::Internal(format!("vector store upsert: {e}")))?;
        }

        self.items.insert(key.to_string(), item);
        Ok(())
    }

    async fn retrieve(&self, key: &str) -> AgentResult<Option<MemoryValue>> {
        Ok(self.items.get(key).map(|item| item.value.clone()))
    }

    async fn remove(&mut self, key: &str) -> AgentResult<bool> {
        let existed = self.items.remove(key).is_some();
        if existed {
            let mut store = self.store.write().await;
            store
                .delete(key)
                .await
                .map_err(|e| AgentError::Internal(format!("vector store delete: {e}")))?;
        }
        Ok(existed)
    }

    /// Search for semantically similar memories using vector similarity.
    ///
    /// The query text is embedded with the same embedder used at store time.
    /// Returns up to `limit` results ranked by cosine similarity (highest first).
    async fn search(&self, query: &str, limit: usize) -> AgentResult<Vec<MemoryItem>> {
        let query_embedding = self.embedder.embed(query).await?;
        let store = self.store.read().await;
        let results = store
            .search(&query_embedding, limit, None)
            .await
            .map_err(|e| AgentError::Internal(format!("vector store search: {e}")))?;

        Ok(results
            .into_iter()
            .filter_map(|r| {
                self.items.get(&r.id).map(|item| {
                    item.clone()
                        .with_score(r.score)
                        .with_metadata("similarity", format!("{:.4}", r.score))
                })
            })
            .collect())
    }

    async fn clear(&mut self) -> AgentResult<()> {
        let mut store = self.store.write().await;
        store
            .clear()
            .await
            .map_err(|e| AgentError::Internal(format!("vector store clear: {e}")))?;
        self.items.clear();
        self.history.clear();
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
        let store = self.store.read().await;
        let total_items = store
            .count()
            .await
            .map_err(|e| AgentError::Internal(format!("vector store count: {e}")))?;
        let total_messages: usize = self.history.values().map(|v| v.len()).sum();
        let total_sessions = self.history.len();
        Ok(MemoryStats {
            total_items,
            total_sessions,
            total_messages,
            memory_bytes: 0,
        })
    }

    fn memory_type(&self) -> &str {
        "semantic"
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // ---- HashEmbedder tests ----

    #[tokio::test]
    async fn test_hash_embedder_dimensions() {
        let embedder = HashEmbedder::new(64);
        let vec = embedder.embed("hello world").await.unwrap();
        assert_eq!(vec.len(), 64);
    }

    #[tokio::test]
    async fn test_hash_embedder_is_normalised() {
        let embedder = HashEmbedder::with_128_dims();
        let vec = embedder.embed("some test text").await.unwrap();
        let norm: f32 = vec.iter().map(|x| x * x).sum::<f32>().sqrt();
        assert!((norm - 1.0).abs() < 1e-5, "embedding should be unit-length, got norm={norm}");
    }

    #[tokio::test]
    async fn test_hash_embedder_similar_texts_closer() {
        let embedder = HashEmbedder::with_128_dims();
        let a = embedder.embed("rust programming language").await.unwrap();
        let b = embedder.embed("rust language systems").await.unwrap();
        let c = embedder.embed("python data science machine learning").await.unwrap();

        let sim_ab: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
        let sim_ac: f32 = a.iter().zip(c.iter()).map(|(x, y)| x * y).sum();

        assert!(
            sim_ab > sim_ac,
            "similar texts should have higher cosine similarity: sim_ab={sim_ab:.4} sim_ac={sim_ac:.4}"
        );
    }

    #[tokio::test]
    async fn test_hash_embedder_empty_text() {
        let embedder = HashEmbedder::with_128_dims();
        let vec = embedder.embed("").await.unwrap();
        assert_eq!(vec.len(), 128);
        // zero vector is fine for empty input
        let norm: f32 = vec.iter().map(|x| x * x).sum::<f32>().sqrt();
        assert_eq!(norm, 0.0);
    }

    #[tokio::test]
    async fn test_hash_embedder_deterministic() {
        let embedder = HashEmbedder::with_128_dims();
        let v1 = embedder.embed("deterministic embedding").await.unwrap();
        let v2 = embedder.embed("deterministic embedding").await.unwrap();
        assert_eq!(v1, v2, "same input must produce identical embeddings");
    }

    // ---- SemanticMemory tests ----

    #[tokio::test]
    async fn test_semantic_memory_store_and_retrieve() {
        let mut mem = SemanticMemory::with_hash_embedder();
        mem.store("k1", MemoryValue::text("hello world")).await.unwrap();
        let val = mem.retrieve("k1").await.unwrap();
        assert!(val.is_some());
        assert_eq!(val.unwrap().as_text(), Some("hello world"));
    }

    #[tokio::test]
    async fn test_semantic_memory_search_returns_relevant() {
        let mut mem = SemanticMemory::with_hash_embedder();
        mem.store("rust", MemoryValue::text("Rust is a systems programming language")).await.unwrap();
        mem.store("python", MemoryValue::text("Python is used in data science")).await.unwrap();
        mem.store("cooking", MemoryValue::text("How to bake a chocolate cake")).await.unwrap();

        let results = mem.search("systems language programming", 2).await.unwrap();
        assert!(!results.is_empty());
        // The rust entry should be in the results
        assert!(results.iter().any(|r| r.key == "rust"));
    }

    #[tokio::test]
    async fn test_semantic_memory_remove() {
        let mut mem = SemanticMemory::with_hash_embedder();
        mem.store("k1", MemoryValue::text("test")).await.unwrap();
        let removed = mem.remove("k1").await.unwrap();
        assert!(removed);
        let val = mem.retrieve("k1").await.unwrap();
        assert!(val.is_none());
    }

    #[tokio::test]
    async fn test_semantic_memory_history() {
        let mut mem = SemanticMemory::with_hash_embedder();
        mem.add_to_history("session-1", Message::user("question")).await.unwrap();
        mem.add_to_history("session-1", Message::assistant("answer")).await.unwrap();

        let history = mem.get_history("session-1").await.unwrap();
        assert_eq!(history.len(), 2);
        assert_eq!(history[0].content, "question");
    }

    #[tokio::test]
    async fn test_semantic_memory_stats() {
        let mut mem = SemanticMemory::with_hash_embedder();
        mem.store("k1", MemoryValue::text("entry one")).await.unwrap();
        mem.store("k2", MemoryValue::text("entry two")).await.unwrap();
        mem.add_to_history("s1", Message::user("msg")).await.unwrap();

        let stats = mem.stats().await.unwrap();
        assert_eq!(stats.total_items, 2);
        assert_eq!(stats.total_sessions, 1);
        assert_eq!(stats.total_messages, 1);
    }

    #[tokio::test]
    async fn test_semantic_memory_clear() {
        let mut mem = SemanticMemory::with_hash_embedder();
        mem.store("k1", MemoryValue::text("data")).await.unwrap();
        mem.add_to_history("s1", Message::user("msg")).await.unwrap();

        mem.clear().await.unwrap();

        let val = mem.retrieve("k1").await.unwrap();
        assert!(val.is_none());
        let history = mem.get_history("s1").await.unwrap();
        assert!(history.is_empty());
    }
}
