//! Semantic Cache Middleware for Gateway Chat Inference
//!
//! Provides vector-based semantic caching for incoming chat prompts to:
//! - Reduce latency on repeated or semantically similar queries
//! - Lower external LLM inference costs
//! - Maintain strict agent tenant boundary isolation
//! - Support configurable similarity thresholds, LRU capacity limits, and TTL expiration

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

/// Trait abstracting embedding computation for semantic cache prompts.
#[async_trait]
pub trait PromptEmbedder: Send + Sync {
    /// Generate an embedding vector for the provided text.
    async fn embed(&self, text: &str) -> Result<Vec<f32>, String>;
}

/// Prompt embedder delegating to MoFA Foundation's `LlmEmbeddingAdapter`.
pub struct LlmPromptEmbedder {
    adapter: mofa_foundation::rag::embedding_adapter::LlmEmbeddingAdapter,
}

impl LlmPromptEmbedder {
    /// Create a new prompt embedder from an `LlmEmbeddingAdapter`.
    pub fn new(adapter: mofa_foundation::rag::embedding_adapter::LlmEmbeddingAdapter) -> Self {
        Self { adapter }
    }
}

#[async_trait]
impl PromptEmbedder for LlmPromptEmbedder {
    async fn embed(&self, text: &str) -> Result<Vec<f32>, String> {
        self.adapter
            .embed_one(text)
            .await
            .map_err(|e| format!("Embedding error: {}", e))
    }
}

/// A deterministic embedder useful for testing and offline development.
pub struct MockPromptEmbedder {
    canned: RwLock<HashMap<String, Vec<f32>>>,
    dimensions: usize,
}

impl MockPromptEmbedder {
    /// Create a new mock embedder with fixed dimensions.
    pub fn new(dimensions: usize) -> Self {
        Self {
            canned: RwLock::new(HashMap::new()),
            dimensions,
        }
    }

    /// Register a canned embedding vector for an exact text match.
    pub fn register(&self, text: impl Into<String>, embedding: Vec<f32>) {
        self.canned.write().insert(text.into(), embedding);
    }
}

#[async_trait]
impl PromptEmbedder for MockPromptEmbedder {
    async fn embed(&self, text: &str) -> Result<Vec<f32>, String> {
        if let Some(vec) = self.canned.read().get(text) {
            return Ok(vec.clone());
        }

        // Generate a deterministic unit vector based on text characters
        let mut vec = vec![0.0f32; self.dimensions];
        for (i, b) in text.as_bytes().iter().enumerate() {
            vec[i % self.dimensions] += (*b as f32) / 255.0;
        }

        let norm: f32 = vec.iter().map(|v| v * v).sum::<f32>().sqrt();
        if norm > 0.0 {
            for v in vec.iter_mut() {
                *v /= norm;
            }
        }
        Ok(vec)
    }
}

/// Compute cosine similarity between two vector slices.
///
/// Returns a value between -1.0 and 1.0 (typically 0.0 to 1.0 for embeddings).
/// Returns 0.0 if vectors have different dimensions or zero magnitude.
pub fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() || a.is_empty() {
        return 0.0;
    }

    let mut dot = 0.0f32;
    let mut norm_a = 0.0f32;
    let mut norm_b = 0.0f32;

    for (x, y) in a.iter().zip(b.iter()) {
        dot += x * y;
        norm_a += x * x;
        norm_b += y * y;
    }

    let denom = (norm_a * norm_b).sqrt();
    if denom <= f32::EPSILON || denom.is_nan() {
        0.0
    } else {
        (dot / denom).clamp(-1.0, 1.0)
    }
}

/// Configuration settings for the Semantic Cache.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SemanticCacheConfig {
    /// Whether the semantic cache is active. Default: false.
    pub enabled: bool,
    /// Cosine similarity threshold required to declare a cache hit (0.0 to 1.0).
    /// Default: 0.92 (production crossover balance between hit rate and false positives).
    pub threshold: f32,
    /// Maximum number of top matches to inspect. Default: 1.
    pub top_k: usize,
    /// Maximum number of cache entries stored per agent before LRU eviction. Default: 1000.
    pub max_entries_per_agent: usize,
    /// Optional time-to-live in seconds. Entries older than this are considered expired.
    /// Default: 3600 (1 hour).
    pub ttl_secs: Option<u64>,
    /// Embedding provider identifier: "openai" or "ollama". Default: "openai".
    pub embedding_provider: String,
    /// Optional model name override (e.g. "text-embedding-3-small").
    pub embedding_model: Option<String>,
}

impl Default for SemanticCacheConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            threshold: 0.92,
            top_k: 1,
            max_entries_per_agent: 1000,
            ttl_secs: Some(3600),
            embedding_provider: "openai".to_string(),
            embedding_model: None,
        }
    }
}

impl SemanticCacheConfig {
    /// Create a new config with enabled set to true and specified similarity threshold.
    pub fn enabled(threshold: f32) -> Self {
        Self {
            enabled: true,
            threshold,
            ..Default::default()
        }
    }
}

/// A single cached response item.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CachedItem {
    /// Original prompt that generated the response.
    pub prompt: String,
    /// Vector embedding of the prompt.
    pub embedding: Vec<f32>,
    /// Stored response output.
    pub output: Value,
    /// Timestamp when item was originally stored.
    pub created_at: DateTime<Utc>,
    /// Timestamp when item was last accessed.
    pub last_accessed_at: DateTime<Utc>,
    /// Total number of times this cached item was hit.
    pub hit_count: u64,
}

/// Successful cache hit information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheHit {
    /// Cached JSON response output.
    pub output: Value,
    /// Cosine similarity score of the match.
    pub similarity: f32,
    /// Original prompt string associated with the cached response.
    pub original_prompt: String,
}

/// Aggregate performance statistics for the semantic cache.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default)]
pub struct CacheStats {
    /// Total cache hits recorded.
    pub hits: u64,
    /// Total cache misses recorded.
    pub misses: u64,
    /// Total number of currently cached entries across all agents.
    pub total_entries: usize,
    /// Current hit ratio (0.0 to 1.0).
    pub hit_ratio: f64,
}

/// High-performance, thread-safe in-memory semantic cache partitioned by agent ID.
pub struct SemanticCache {
    config: SemanticCacheConfig,
    embedder: Arc<dyn PromptEmbedder>,
    /// Map of `agent_id` -> list of cached items.
    store: RwLock<HashMap<String, Vec<CachedItem>>>,
    hits: AtomicU64,
    misses: AtomicU64,
}

impl SemanticCache {
    /// Create a new `SemanticCache` with the given configuration and embedder.
    pub fn new(config: SemanticCacheConfig, embedder: Arc<dyn PromptEmbedder>) -> Self {
        Self {
            config,
            embedder,
            store: RwLock::new(HashMap::new()),
            hits: AtomicU64::new(0),
            misses: AtomicU64::new(0),
        }
    }

    /// Check if the semantic cache is enabled.
    pub fn is_enabled(&self) -> bool {
        self.config.enabled
    }

    /// Access current configuration.
    pub fn config(&self) -> &SemanticCacheConfig {
        &self.config
    }

    /// Check the cache for a semantically similar prompt for the specified agent.
    ///
    /// If a match with cosine similarity >= `config.threshold` is found and has not
    /// expired per `ttl_secs`, its output is returned and access metadata is updated.
    pub async fn lookup(&self, agent_id: &str, prompt: &str) -> Option<CacheHit> {
        if !self.config.enabled || prompt.trim().is_empty() {
            return None;
        }

        // Compute prompt embedding
        let query_embedding = match self.embedder.embed(prompt).await {
            Ok(vec) => vec,
            Err(err) => {
                tracing::warn!(error = %err, "Failed to compute embedding for semantic cache lookup");
                self.misses.fetch_add(1, Ordering::Relaxed);
                return None;
            }
        };

        let now = Utc::now();
        let ttl_duration = self.config.ttl_secs.map(Duration::from_secs);

        // Read lock to find best candidate
        let match_result = {
            let store = self.store.read();
            if let Some(items) = store.get(agent_id) {
                let mut best_match: Option<(usize, f32)> = None;

                for (idx, item) in items.iter().enumerate() {
                    // Check TTL expiration
                    if let Some(ttl) = ttl_duration {
                        if let Ok(age) = (now - item.created_at).to_std() {
                            if age > ttl {
                                continue;
                            }
                        }
                    }

                    let sim = cosine_similarity(&query_embedding, &item.embedding);
                    if sim >= self.config.threshold {
                        match best_match {
                            Some((_, best_sim)) if sim > best_sim => {
                                best_match = Some((idx, sim));
                            }
                            None => {
                                best_match = Some((idx, sim));
                            }
                            _ => {}
                        }
                    }
                }

                best_match
            } else {
                None
            }
        };

        if let Some((best_idx, sim)) = match_result {
            let mut store = self.store.write();
            if let Some(items) = store.get_mut(agent_id) {
                if let Some(item) = items.get_mut(best_idx) {
                    item.last_accessed_at = now;
                    item.hit_count = item.hit_count.saturating_add(1);

                    self.hits.fetch_add(1, Ordering::Relaxed);
                    return Some(CacheHit {
                        output: item.output.clone(),
                        similarity: sim,
                        original_prompt: item.prompt.clone(),
                    });
                }
            }
        }

        self.misses.fetch_add(1, Ordering::Relaxed);
        None
    }

    /// Insert a newly executed prompt and its output into the cache.
    ///
    /// Enforces TTL pruning and LRU capacity limits per agent.
    pub async fn insert(&self, agent_id: &str, prompt: &str, output: Value) -> Result<(), String> {
        if !self.config.enabled || prompt.trim().is_empty() {
            return Ok(());
        }

        let embedding = self.embedder.embed(prompt).await?;
        let now = Utc::now();

        let new_item = CachedItem {
            prompt: prompt.to_string(),
            embedding,
            output,
            created_at: now,
            last_accessed_at: now,
            hit_count: 0,
        };

        let mut store = self.store.write();
        let items = store.entry(agent_id.to_string()).or_default();

        // 1. Prune expired entries if TTL is configured
        if let Some(ttl) = self.config.ttl_secs.map(Duration::from_secs) {
            items.retain(|item| {
                if let Ok(age) = (now - item.created_at).to_std() {
                    age <= ttl
                } else {
                    true
                }
            });
        }

        // 2. Enforce LRU eviction if capacity exceeded
        while items.len() >= self.config.max_entries_per_agent && !items.is_empty() {
            // Find oldest by last_accessed_at
            let mut oldest_idx = 0;
            let mut oldest_time = items[0].last_accessed_at;

            for (idx, item) in items.iter().enumerate().skip(1) {
                if item.last_accessed_at < oldest_time {
                    oldest_time = item.last_accessed_at;
                    oldest_idx = idx;
                }
            }

            items.swap_remove(oldest_idx);
        }

        items.push(new_item);
        Ok(())
    }

    /// Retrieve aggregate statistics.
    pub fn stats(&self) -> CacheStats {
        let hits = self.hits.load(Ordering::Relaxed);
        let misses = self.misses.load(Ordering::Relaxed);
        let total_requests = hits + misses;
        let hit_ratio = if total_requests > 0 {
            hits as f64 / total_requests as f64
        } else {
            0.0
        };

        let total_entries = {
            let store = self.store.read();
            store.values().map(|v| v.len()).sum()
        };

        CacheStats {
            hits,
            misses,
            total_entries,
            hit_ratio,
        }
    }

    /// Clear all cached items across all agents.
    pub fn clear(&self) {
        let mut store = self.store.write();
        store.clear();
    }

    /// Clear all cached items for a specific agent.
    pub fn clear_agent(&self, agent_id: &str) {
        let mut store = self.store.write();
        store.remove(agent_id);
    }

    /// Return total cached items count across all agents.
    pub fn len(&self) -> usize {
        let store = self.store.read();
        store.values().map(|v| v.len()).sum()
    }

    /// Return whether the cache has zero entries.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Return count of entries for a specific agent.
    pub fn agent_entry_count(&self, agent_id: &str) -> usize {
        let store = self.store.read();
        store.get(agent_id).map(|v| v.len()).unwrap_or(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[tokio::test]
    async fn test_cosine_similarity_computation() {
        let v1 = vec![1.0, 0.0, 0.0];
        let v2 = vec![1.0, 0.0, 0.0];
        assert!((cosine_similarity(&v1, &v2) - 1.0).abs() < 1e-6);

        let v3 = vec![0.0, 1.0, 0.0];
        assert!(cosine_similarity(&v1, &v3).abs() < 1e-6);

        let v4 = vec![1.0, 1.0, 0.0];
        let expected = 1.0 / 2.0f32.sqrt();
        assert!((cosine_similarity(&v1, &v4) - expected).abs() < 1e-6);
    }

    #[tokio::test]
    async fn test_semantic_cache_hit_and_miss() {
        let embedder = Arc::new(MockPromptEmbedder::new(3));
        embedder.register("What is MoFA?", vec![1.0, 0.0, 0.0]);
        // Near-identical prompt: [0.99, 0.14, 0.0] -> cosine sim ~ 0.99
        embedder.register("What is MoFA framework?", vec![0.99, 0.14, 0.0]);
        // Completely different prompt
        embedder.register("How is the weather today?", vec![0.0, 1.0, 0.0]);

        let config = SemanticCacheConfig {
            enabled: true,
            threshold: 0.90,
            ..Default::default()
        };
        let cache = SemanticCache::new(config, embedder);

        // Initially empty: miss
        let hit = cache.lookup("agent-1", "What is MoFA?").await;
        assert!(hit.is_none());

        // Insert response
        cache
            .insert(
                "agent-1",
                "What is MoFA?",
                json!({"reply": "MoFA is a multi-agent framework"}),
            )
            .await
            .unwrap();

        // Exact query: hit
        let hit = cache.lookup("agent-1", "What is MoFA?").await.unwrap();
        assert_eq!(hit.output["reply"], "MoFA is a multi-agent framework");
        assert!(hit.similarity >= 0.99);

        // Near-identical query: hit
        let hit = cache
            .lookup("agent-1", "What is MoFA framework?")
            .await
            .unwrap();
        assert_eq!(hit.output["reply"], "MoFA is a multi-agent framework");
        assert!(hit.similarity >= 0.90);

        // Different query: miss
        let hit = cache
            .lookup("agent-1", "How is the weather today?")
            .await;
        assert!(hit.is_none());

        let stats = cache.stats();
        assert_eq!(stats.hits, 2);
        assert_eq!(stats.misses, 2);
    }

    #[tokio::test]
    async fn test_agent_boundary_isolation() {
        let embedder = Arc::new(MockPromptEmbedder::new(3));
        embedder.register("secret query", vec![1.0, 0.0, 0.0]);

        let config = SemanticCacheConfig::enabled(0.90);
        let cache = SemanticCache::new(config, embedder);

        // Insert for agent-a
        cache
            .insert("agent-a", "secret query", json!({"data": "agent-a-secret"}))
            .await
            .unwrap();

        // agent-a can hit
        let hit_a = cache.lookup("agent-a", "secret query").await;
        assert!(hit_a.is_some());
        assert_eq!(hit_a.unwrap().output["data"], "agent-a-secret");

        // agent-b looking up identical prompt MUST miss (no cross-agent leakage)
        let hit_b = cache.lookup("agent-b", "secret query").await;
        assert!(hit_b.is_none(), "Cross-agent cache leakage detected!");
    }

    #[tokio::test]
    async fn test_disabled_mode_bypasses_cache() {
        let embedder = Arc::new(MockPromptEmbedder::new(3));
        embedder.register("query", vec![1.0, 0.0, 0.0]);

        let config = SemanticCacheConfig {
            enabled: false, // disabled
            ..Default::default()
        };
        let cache = SemanticCache::new(config, embedder);

        // Insert should be no-op
        cache
            .insert("agent-1", "query", json!({"reply": "hello"}))
            .await
            .unwrap();
        assert_eq!(cache.len(), 0);

        // Lookup should return None
        assert!(cache.lookup("agent-1", "query").await.is_none());
    }

    #[tokio::test]
    async fn test_lru_eviction_when_capacity_exceeded() {
        let embedder = Arc::new(MockPromptEmbedder::new(3));
        embedder.register("q1", vec![1.0, 0.0, 0.0]);
        embedder.register("q2", vec![0.0, 1.0, 0.0]);
        embedder.register("q3", vec![0.0, 0.0, 1.0]);

        let mut config = SemanticCacheConfig::enabled(0.90);
        config.max_entries_per_agent = 2; // only 2 items allowed
        let cache = SemanticCache::new(config, embedder);

        cache.insert("agent-1", "q1", json!("a1")).await.unwrap();
        tokio::time::sleep(Duration::from_millis(5)).await;
        cache.insert("agent-1", "q2", json!("a2")).await.unwrap();

        assert_eq!(cache.agent_entry_count("agent-1"), 2);

        // Access q1 so q2 becomes the least recently used
        tokio::time::sleep(Duration::from_millis(5)).await;
        let _ = cache.lookup("agent-1", "q1").await;

        // Insert q3, which should evict q2
        tokio::time::sleep(Duration::from_millis(5)).await;
        cache.insert("agent-1", "q3", json!("a3")).await.unwrap();

        assert_eq!(cache.agent_entry_count("agent-1"), 2);
        assert!(cache.lookup("agent-1", "q1").await.is_some());
        assert!(cache.lookup("agent-1", "q3").await.is_some());
        assert!(cache.lookup("agent-1", "q2").await.is_none());
    }
}

