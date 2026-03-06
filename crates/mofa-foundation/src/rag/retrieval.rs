//! RAG Retrieval Pipeline
//!
//! Query-time pipeline that connects:
//! 1. Query embedding via [`LlmEmbeddingAdapter`]
//! 2. ANN search against a vector store
//! 3. Metadata-based post-filtering
//! 4. Score-based reranking
//! 5. Context packing within a byte budget
//!
//! This module provides the "retrieve context for this query" entry point.

use crate::rag::embedding_adapter::{EmbeddingAdapterError, LlmEmbeddingAdapter};
use mofa_kernel::rag::{SearchResult, VectorStore};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

/// Errors from the RAG orchestration layer.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum RagOrchestrationError {
    /// Embedding adapter error.
    #[error("embedding error: {0}")]
    Embedding(#[from] EmbeddingAdapterError),

    /// Vector store error (stringified — VectorStore trait returns AgentError
    /// which doesn't implement std::error::Error, so we store the message).
    #[error("vector store error: {0}")]
    VectorStore(String),

    /// Invalid input.
    #[error("invalid input: {0}")]
    InvalidInput(String),
}

// ---------------------------------------------------------------------------
// Retrieval types
// ---------------------------------------------------------------------------

/// Configuration for the retrieval pipeline.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RagQueryConfig {
    /// Number of results to retrieve from ANN search.
    pub top_k: usize,
    /// Minimum similarity score threshold.
    pub threshold: Option<f32>,
    /// Number of top results to keep after reranking.
    /// When `None`, keeps all post-filter results (no extra truncation).
    pub rerank_top_k: Option<usize>,
    /// Maximum total bytes for packed context output.
    /// When `None`, no budget is enforced.
    /// Note: this counts UTF-8 bytes, not Unicode characters.
    pub max_context_bytes: Option<usize>,
    /// Optional metadata filter: only results with ALL matching key-value pairs.
    pub metadata_filter: HashMap<String, String>,
}

impl Default for RagQueryConfig {
    fn default() -> Self {
        Self {
            top_k: 5,
            threshold: None,
            rerank_top_k: None,
            max_context_bytes: None,
            metadata_filter: HashMap::new(),
        }
    }
}

impl RagQueryConfig {
    /// Builder: set top_k.
    pub fn with_top_k(mut self, k: usize) -> Self {
        self.top_k = k.max(1);
        self
    }

    /// Builder: set similarity threshold.
    pub fn with_threshold(mut self, t: f32) -> Self {
        self.threshold = Some(t);
        self
    }

    /// Builder: set rerank_top_k.
    pub fn with_rerank_top_k(mut self, k: usize) -> Self {
        self.rerank_top_k = Some(k.max(1));
        self
    }

    /// Builder: set max context bytes.
    pub fn with_max_context_chars(mut self, bytes: usize) -> Self {
        self.max_context_bytes = Some(bytes);
        self
    }

    /// Builder: add a metadata filter.
    pub fn with_filter(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.metadata_filter.insert(key.into(), value.into());
        self
    }
}

/// A retrieved chunk with its similarity score and metadata.
#[derive(Debug, Clone)]
pub struct RetrievedChunk {
    /// Chunk identifier.
    pub id: String,
    /// Chunk text content.
    pub text: String,
    /// Similarity score from ANN search.
    pub score: f32,
    /// Chunk metadata.
    pub metadata: HashMap<String, String>,
}

impl From<SearchResult> for RetrievedChunk {
    fn from(sr: SearchResult) -> Self {
        Self {
            id: sr.id,
            text: sr.text,
            score: sr.score,
            metadata: sr.metadata,
        }
    }
}

/// Result of a retrieval query.
#[derive(Debug, Clone)]
pub struct RetrievalResult {
    /// The original query.
    pub query: String,
    /// Retrieved chunks (filtered, reranked, and budget-trimmed).
    pub chunks: Vec<RetrievedChunk>,
    /// Packed context string (chunks joined with separator).
    pub context: String,
    /// Total bytes in the packed context.
    pub context_bytes: usize,
}

// ---------------------------------------------------------------------------
// Query pipeline
// ---------------------------------------------------------------------------

/// Query the vector store with embedding + search + filter + rerank + pack.
///
/// 1. Embed the query text
/// 2. ANN search in the vector store
/// 3. Apply metadata filters
/// 4. Rerank by score (top rerank_top_k)
/// 5. Pack results within token budget
///
/// Returns a [`RetrievalResult`] with chunks and packed context.
pub async fn query_documents<S: VectorStore>(
    store: &S,
    embedder: &LlmEmbeddingAdapter,
    query: &str,
    config: &RagQueryConfig,
) -> Result<RetrievalResult, RagOrchestrationError> {
    let query = query.trim();
    if query.is_empty() {
        return Err(RagOrchestrationError::InvalidInput(
            "query must not be empty".to_string(),
        ));
    }

    // 1. Embed query
    let query_embedding = embedder
        .embed_one(query)
        .await
        .map_err(RagOrchestrationError::Embedding)?;

    // 2. ANN search -- over-fetch when filtering is active to compensate
    // for post-filter attrition. saturating_mul prevents overflow.
    let fetch_k = if config.metadata_filter.is_empty() {
        config.top_k
    } else {
        config.top_k.saturating_mul(3)
    };

    let search_results = store
        .search(&query_embedding, fetch_k, config.threshold)
        .await
        .map_err(|e| RagOrchestrationError::VectorStore(e.to_string()))?;

    // 3. Metadata filter
    let filtered: Vec<RetrievedChunk> = search_results
        .into_iter()
        .map(RetrievedChunk::from)
        .filter(|chunk| {
            config.metadata_filter.iter().all(|(key, value)| {
                chunk.metadata.get(key).map(|v| v == value).unwrap_or(false)
            })
        })
        .collect();

    // 4. Rerank (score-based, already sorted by ANN search)
    // When rerank_top_k is None, keep all post-filter results.
    let ranked: Vec<RetrievedChunk> = if let Some(limit) = config.rerank_top_k {
        filtered.into_iter().take(limit).collect()
    } else {
        filtered
    };

    // 5. Context packing within budget
    let (packed_chunks, context) = pack_context(&ranked, config.max_context_bytes);
    let context_bytes = context.len();

    Ok(RetrievalResult {
        query: query.to_string(),
        chunks: packed_chunks,
        context,
        context_bytes,
    })
}

/// Pack retrieved chunks into a single context string, respecting
/// an optional byte budget. Note: budget counts UTF-8 bytes, not chars.
fn pack_context(
    chunks: &[RetrievedChunk],
    max_chars: Option<usize>,
) -> (Vec<RetrievedChunk>, String) {
    const SEPARATOR: &str = "\n\n---\n\n";

    let mut packed = Vec::new();
    let mut parts = Vec::new();
    let mut total_chars: usize = 0;

    for chunk in chunks {
        let separator_cost = if parts.is_empty() { 0 } else { SEPARATOR.len() };
        let cost = chunk.text.len() + separator_cost;

        if let Some(budget) = max_chars {
            if total_chars + cost > budget && !packed.is_empty() {
                break;
            }
        }

        packed.push(chunk.clone());
        parts.push(chunk.text.as_str());
        total_chars += cost;
    }

    let context = parts.join(SEPARATOR);
    (packed, context)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rag::embedding_adapter::RagEmbeddingConfig;
    use async_trait::async_trait;
    use mofa_kernel::agent::error::{AgentError, AgentResult};
    use mofa_kernel::rag::{DocumentChunk, SimilarityMetric};

    // -- Mock embedder --

    struct MockProvider { dimensions: usize }

    #[async_trait]
    impl crate::llm::provider::LLMProvider for MockProvider {
        fn name(&self) -> &str { "mock" }
        fn default_model(&self) -> &str { "mock-embed" }
        fn supports_streaming(&self) -> bool { false }
        fn supports_tools(&self) -> bool { false }
        fn supports_vision(&self) -> bool { false }

        async fn chat(
            &self, _r: crate::llm::types::ChatCompletionRequest,
        ) -> crate::llm::types::LLMResult<crate::llm::types::ChatCompletionResponse> {
            Err(crate::llm::types::LLMError::Other("not supported".into()))
        }
        async fn chat_stream(
            &self, _r: crate::llm::types::ChatCompletionRequest,
        ) -> crate::llm::types::LLMResult<crate::llm::provider::ChatStream> {
            Err(crate::llm::types::LLMError::Other("not supported".into()))
        }
        async fn embedding(
            &self, request: crate::llm::types::EmbeddingRequest,
        ) -> crate::llm::types::LLMResult<crate::llm::types::EmbeddingResponse> {
            let inputs = match request.input {
                crate::llm::types::EmbeddingInput::Single(s) => vec![s],
                crate::llm::types::EmbeddingInput::Multiple(v) => v,
            };
            let data = inputs.iter().map(|text| {
                let mut vec = vec![0.0f32; self.dimensions];
                for (i, b) in text.bytes().enumerate() {
                    vec[i % self.dimensions] += b as f32 / 255.0;
                }
                let norm = vec.iter().map(|x| x * x).sum::<f32>().sqrt();
                if norm > 0.0 { for v in &mut vec { *v /= norm; } }
                crate::llm::types::EmbeddingData {
                    object: "embedding".into(), embedding: vec, index: 0,
                }
            }).collect();
            Ok(crate::llm::types::EmbeddingResponse {
                object: "list".into(), data, model: request.model,
                usage: crate::llm::types::EmbeddingUsage { prompt_tokens: 0, total_tokens: 0 },
            })
        }
    }

    fn make_adapter(dim: usize) -> LlmEmbeddingAdapter {
        let provider = std::sync::Arc::new(MockProvider { dimensions: dim });
        let client = crate::llm::client::LLMClient::new(provider);
        LlmEmbeddingAdapter::new(client, RagEmbeddingConfig::default().with_dimensions(dim))
    }

    // -- Mock vector store --

    struct TestStore {
        chunks: HashMap<String, DocumentChunk>,
        dimensions: Option<usize>,
    }
    impl TestStore { fn new() -> Self { Self { chunks: HashMap::new(), dimensions: None } } }

    #[async_trait]
    impl VectorStore for TestStore {
        async fn upsert(&mut self, chunk: DocumentChunk) -> AgentResult<()> {
            if let Some(d) = self.dimensions {
                if chunk.embedding.len() != d {
                    return Err(AgentError::InvalidInput(format!(
                        "dim mismatch: {} vs {}", d, chunk.embedding.len())));
                }
            } else { self.dimensions = Some(chunk.embedding.len()); }
            self.chunks.insert(chunk.id.clone(), chunk);
            Ok(())
        }
        async fn search(&self, qe: &[f32], top_k: usize, threshold: Option<f32>) -> AgentResult<Vec<SearchResult>> {
            let mut results: Vec<SearchResult> = self.chunks.values().map(|c| {
                let score: f32 = c.embedding.iter().zip(qe.iter()).map(|(a, b)| a * b).sum();
                SearchResult { id: c.id.clone(), text: c.text.clone(), score, metadata: c.metadata.clone() }
            }).filter(|r| threshold.map(|t| r.score >= t).unwrap_or(true)).collect();
            results.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
            results.truncate(top_k);
            Ok(results)
        }
        async fn delete(&mut self, id: &str) -> AgentResult<bool> { Ok(self.chunks.remove(id).is_some()) }
        async fn clear(&mut self) -> AgentResult<()> { self.chunks.clear(); self.dimensions = None; Ok(()) }
        async fn count(&self) -> AgentResult<usize> { Ok(self.chunks.len()) }
        fn similarity_metric(&self) -> SimilarityMetric { SimilarityMetric::DotProduct }
    }

    /// Helper: embed text and insert into a TestStore
    async fn seed_store(store: &mut TestStore, adapter: &LlmEmbeddingAdapter, entries: &[(&str, &str, &[(&str, &str)])]) {
        for (id, text, meta) in entries {
            let emb = adapter.embed_one(text).await.unwrap();
            let mut chunk = DocumentChunk::new(*id, *text, emb);
            for (k, v) in *meta {
                chunk = chunk.with_metadata(*k, *v);
            }
            store.upsert(chunk).await.unwrap();
        }
    }

    // -- Query tests --

    #[tokio::test]
    async fn query_empty_store() {
        let store = TestStore::new();
        let adapter = make_adapter(16);
        let result = query_documents(&store, &adapter, "hello", &RagQueryConfig::default()).await.unwrap();
        assert!(result.chunks.is_empty());
        assert!(result.context.is_empty());
    }

    #[tokio::test]
    async fn query_returns_relevant_results() {
        let mut store = TestStore::new();
        let adapter = make_adapter(16);
        seed_store(&mut store, &adapter, &[
            ("rust-0", "Rust programming language systems", &[]),
            ("python-0", "Python scripting language", &[]),
        ]).await;

        let result = query_documents(&store, &adapter, "Rust systems", &RagQueryConfig::default().with_top_k(2)).await.unwrap();
        assert!(!result.chunks.is_empty());
        assert!(!result.context.is_empty());
        assert_eq!(result.query, "Rust systems");
    }

    #[tokio::test]
    async fn query_rejects_empty_query() {
        let store = TestStore::new();
        let adapter = make_adapter(16);
        let err = query_documents(&store, &adapter, "  ", &RagQueryConfig::default()).await.unwrap_err();
        assert!(matches!(err, RagOrchestrationError::InvalidInput(_)));
    }

    #[tokio::test]
    async fn query_with_metadata_filter() {
        let mut store = TestStore::new();
        let adapter = make_adapter(16);
        seed_store(&mut store, &adapter, &[
            ("a", "Rust lang", &[("category", "systems")]),
            ("b", "Python lang", &[("category", "scripting")]),
        ]).await;

        let config = RagQueryConfig::default().with_top_k(10).with_filter("category", "systems");
        let result = query_documents(&store, &adapter, "language", &config).await.unwrap();
        for chunk in &result.chunks {
            assert_eq!(chunk.metadata.get("category").map(String::as_str), Some("systems"));
        }
    }

    #[tokio::test]
    async fn query_respects_context_budget() {
        let mut store = TestStore::new();
        let adapter = make_adapter(16);
        // Seed multiple chunks so budget can trim
        seed_store(&mut store, &adapter, &[
            ("c1", "AAAAAAAAAA", &[]),
            ("c2", "BBBBBBBBBB", &[]),
            ("c3", "CCCCCCCCCC", &[]),
        ]).await;

        let config = RagQueryConfig::default().with_top_k(10).with_max_context_chars(15);
        let result = query_documents(&store, &adapter, "AAAA", &config).await.unwrap();
        assert!(result.context_bytes <= 15 || result.chunks.len() == 1);
    }

    // -- pack_context tests --

    #[test]
    fn pack_context_empty() {
        let (chunks, ctx) = pack_context(&[], None);
        assert!(chunks.is_empty());
        assert!(ctx.is_empty());
    }

    #[test]
    fn pack_context_single_chunk() {
        let chunks = vec![RetrievedChunk { id: "1".into(), text: "Hello".into(), score: 1.0, metadata: HashMap::new() }];
        let (packed, ctx) = pack_context(&chunks, None);
        assert_eq!(packed.len(), 1);
        assert_eq!(ctx, "Hello");
    }

    #[test]
    fn pack_context_multiple_with_separator() {
        let chunks = vec![
            RetrievedChunk { id: "1".into(), text: "First".into(), score: 1.0, metadata: HashMap::new() },
            RetrievedChunk { id: "2".into(), text: "Second".into(), score: 0.9, metadata: HashMap::new() },
        ];
        let (packed, ctx) = pack_context(&chunks, None);
        assert_eq!(packed.len(), 2);
        assert!(ctx.contains("First"));
        assert!(ctx.contains("Second"));
        assert!(ctx.contains("---"));
    }

    #[test]
    fn pack_context_budget_truncates() {
        let chunks = vec![
            RetrievedChunk { id: "1".into(), text: "Short".into(), score: 1.0, metadata: HashMap::new() },
            RetrievedChunk { id: "2".into(), text: "Much longer text here".into(), score: 0.9, metadata: HashMap::new() },
        ];
        let (packed, _) = pack_context(&chunks, Some(8));
        assert_eq!(packed.len(), 1);
    }

    #[test]
    fn query_config_defaults() {
        let c = RagQueryConfig::default();
        assert_eq!(c.top_k, 5);
        assert!(c.threshold.is_none());
        assert!(c.rerank_top_k.is_none());
        assert!(c.max_context_bytes.is_none());
        assert!(c.metadata_filter.is_empty());
    }
}
