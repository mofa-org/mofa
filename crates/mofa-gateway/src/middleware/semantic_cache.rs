//! Semantic cache middleware primitives.
//!
//! The cache embeds incoming prompts and performs similarity search against
//! recently answered prompts. If a close match is found, the stored response
//! is returned without invoking the LLM again.

use async_trait::async_trait;
use mofa_foundation::llm::client::LLMClient;
use mofa_foundation::llm::ollama::{OllamaConfig, OllamaProvider};
use mofa_foundation::llm::openai::{OpenAIConfig, OpenAIProvider};
use mofa_foundation::rag::embedding_adapter::{
    EmbeddingAdapterError, LlmEmbeddingAdapter, RagEmbeddingConfig, RagEmbeddingProvider,
    deterministic_chunk_id,
};
use mofa_foundation::rag::vector_store::InMemoryVectorStore;
use mofa_kernel::rag::{DocumentChunk, SimilarityMetric, VectorStore};
use serde_json::Value;
use std::sync::Arc;
use thiserror::Error;
use tokio::sync::RwLock;

const META_AGENT_ID: &str = "agent_id";
const META_RESPONSE_JSON: &str = "response_json";

/// Runtime configuration for semantic cache behavior.
#[derive(Debug, Clone)]
pub struct SemanticCacheConfig {
    /// Enable semantic cache lookups and writes.
    pub enabled: bool,
    /// Minimum cosine similarity to treat as a cache hit.
    pub similarity_threshold: f32,
    /// Number of nearest neighbors to inspect per query.
    pub search_top_k: usize,
    /// Embedding adapter configuration.
    pub embedding: RagEmbeddingConfig,
}


impl Default for SemanticCacheConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            similarity_threshold: 0.95,
            search_top_k: 8,
            embedding: RagEmbeddingConfig::default(),
        }
    }
}

impl SemanticCacheConfig {
    fn normalized_threshold(&self) -> f32 {
        self.similarity_threshold.clamp(-1.0, 1.0)
    }

    fn normalized_top_k(&self) -> usize {
        self.search_top_k.max(1)
    }
}

/// A semantic cache hit with matched response and score.
#[derive(Debug, Clone)]
pub struct SemanticCacheHit {
    /// Cached response payload.
    pub output: Value,
    /// Similarity score of the matched prompt.
    pub score: f32,
}

/// Errors returned by semantic cache operations.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum SemanticCacheError {
    /// Embedding generation failed.
    #[error("embedding failed: {0}")]
    Embedding(#[from] EmbeddingAdapterError),

    /// JSON serialization or deserialization failed.
    #[error("json serialization failed: {0}")]
    Json(#[from] serde_json::Error),

    /// Vector store operation failed.
    #[error("vector store operation failed: {0}")]
    Store(String),
}

#[async_trait]
trait PromptEmbedder: Send + Sync {
    async fn embed(&self, text: &str) -> Result<Vec<f32>, SemanticCacheError>;
}

#[async_trait]
impl PromptEmbedder for LlmEmbeddingAdapter {
    async fn embed(&self, text: &str) -> Result<Vec<f32>, SemanticCacheError> {
        self.embed_one(text).await.map_err(SemanticCacheError::from)
    }
}

/// Semantic cache backed by vector similarity search.
pub struct SemanticCache {
    config: SemanticCacheConfig,
    embedder: Arc<dyn PromptEmbedder>,
    store: Arc<RwLock<InMemoryVectorStore>>,
}

impl SemanticCache {
    /// Build a semantic cache using provider settings from `config.embedding`.
    pub fn from_config(config: SemanticCacheConfig) -> Self {
        let provider: Arc<dyn mofa_foundation::llm::provider::LLMProvider> =
            match config.embedding.provider {
                RagEmbeddingProvider::OpenAi => {
                    let mut openai_cfg = OpenAIConfig::from_env();
                    if let Some(model) = &config.embedding.model {
                        openai_cfg = openai_cfg.with_model(model.clone());
                    }
                    Arc::new(OpenAIProvider::with_config(openai_cfg))
                }
                RagEmbeddingProvider::Ollama => {
                    let mut ollama_cfg = OllamaConfig::from_env();
                    if let Some(model) = &config.embedding.model {
                        ollama_cfg = ollama_cfg.with_model(model.clone());
                    }
                    Arc::new(OllamaProvider::with_config(ollama_cfg))
                }
                _ => {
                    tracing::warn!(
                        "unknown embedding provider variant; falling back to OpenAI provider"
                    );
                    Arc::new(OpenAIProvider::from_env())
                }
            };

        let embedder = Arc::new(LlmEmbeddingAdapter::new(
            LLMClient::new(provider),
            config.embedding.clone(),
        ));
        Self::with_embedder(config, embedder)
    }

    fn with_embedder(config: SemanticCacheConfig, embedder: Arc<dyn PromptEmbedder>) -> Self {
        Self {
            config,
            embedder,
            store: Arc::new(RwLock::new(InMemoryVectorStore::new(SimilarityMetric::Cosine))),
        }
    }

    /// Search the cache for an existing response to a semantically similar prompt.
    pub async fn lookup(
        &self,
        agent_id: &str,
        prompt: &str,
    ) -> Result<Option<SemanticCacheHit>, SemanticCacheError> {
        if !self.config.enabled {
            return Ok(None);
        }

        let prompt = prompt.trim();
        if prompt.is_empty() {
            return Ok(None);
        }

        let embedding = self.embedder.embed(prompt).await?;
        let results = {
            let store = self.store.read().await;
            store
                .search(
                    &embedding,
                    self.config.normalized_top_k(),
                    Some(self.config.normalized_threshold()),
                )
                .await
                .map_err(|e| SemanticCacheError::Store(e.to_string()))?
        };

        for result in results {
            if result.metadata.get(META_AGENT_ID).map(String::as_str) != Some(agent_id) {
                continue;
            }

            let Some(raw_output) = result.metadata.get(META_RESPONSE_JSON) else {
                continue;
            };

            let output: Value = serde_json::from_str(raw_output)?;
            return Ok(Some(SemanticCacheHit {
                output,
                score: result.score,
            }));
        }

        Ok(None)
    }

    /// Insert or update a cache entry for the given prompt/output pair.
    pub async fn insert(
        &self,
        agent_id: &str,
        prompt: &str,
        output: &Value,
    ) -> Result<(), SemanticCacheError> {
        if !self.config.enabled {
            return Ok(());
        }

        let prompt = prompt.trim();
        if prompt.is_empty() {
            return Ok(());
        }

        let embedding = self.embedder.embed(prompt).await?;
        let response_json = serde_json::to_string(output)?;

        let chunk_id = deterministic_chunk_id(agent_id, 0, prompt);
        let chunk = DocumentChunk::new(chunk_id, prompt, embedding)
            .with_metadata(META_AGENT_ID, agent_id)
            .with_metadata(META_RESPONSE_JSON, response_json);

        let mut store = self.store.write().await;
        store
            .upsert(chunk)
            .await
            .map_err(|e| SemanticCacheError::Store(e.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    struct FakeEmbedder {
        vectors: HashMap<String, Vec<f32>>,
    }

    #[async_trait]
    impl PromptEmbedder for FakeEmbedder {
        async fn embed(&self, text: &str) -> Result<Vec<f32>, SemanticCacheError> {
            Ok(self
                .vectors
                .get(text)
                .cloned()
                .unwrap_or_else(|| vec![0.0_f32, 1.0_f32]))
        }
    }

    fn test_cache_with_vectors(vectors: HashMap<String, Vec<f32>>) -> SemanticCache {
        let config = SemanticCacheConfig {
            enabled: true,
            similarity_threshold: 0.95,
            search_top_k: 5,
            embedding: RagEmbeddingConfig::default(),
        };
        SemanticCache::with_embedder(config, Arc::new(FakeEmbedder { vectors }))
    }

    #[tokio::test]
    async fn returns_cached_response_for_semantic_match() {
        let mut vectors = HashMap::new();
        vectors.insert("hello world".to_string(), vec![1.0, 0.0]);
        vectors.insert("hi world".to_string(), vec![0.99, 0.01]);
        let cache = test_cache_with_vectors(vectors);

        let response = serde_json::json!({"answer": "cached"});
        cache
            .insert("agent-a", "hello world", &response)
            .await
            .expect("cache insert should succeed");

        let hit = cache
            .lookup("agent-a", "hi world")
            .await
            .expect("lookup should succeed")
            .expect("expected cache hit");

        assert_eq!(hit.output, response);
        assert!(hit.score >= 0.95);
    }

    #[tokio::test]
    async fn does_not_cross_agent_boundaries() {
        let mut vectors = HashMap::new();
        vectors.insert("retry payment".to_string(), vec![1.0, 0.0]);
        vectors.insert("payment failed retry".to_string(), vec![0.99, 0.01]);
        let cache = test_cache_with_vectors(vectors);

        let response = serde_json::json!({"answer": "billing guidance"});
        cache
            .insert("billing-agent", "retry payment", &response)
            .await
            .expect("cache insert should succeed");

        let miss = cache
            .lookup("support-agent", "payment failed retry")
            .await
            .expect("lookup should succeed");

        assert!(miss.is_none());
    }

    #[tokio::test]
    async fn disabled_cache_never_hits_or_writes() {
        let config = SemanticCacheConfig {
            enabled: false,
            ..SemanticCacheConfig::default()
        };

        let cache = SemanticCache::with_embedder(
            config,
            Arc::new(FakeEmbedder {
                vectors: HashMap::new(),
            }),
        );

        let output = serde_json::json!({"answer": "x"});
        cache
            .insert("agent-a", "hello", &output)
            .await
            .expect("disabled insert should be no-op");

        let hit = cache
            .lookup("agent-a", "hello")
            .await
            .expect("disabled lookup should succeed");

        assert!(hit.is_none());
    }
}