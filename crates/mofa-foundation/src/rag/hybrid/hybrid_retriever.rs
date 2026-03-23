//! Hybrid Retriever implementation
//!
//! Provides a concrete implementation of the HybridRetriever trait that
//! combines dense vector search with BM25 sparse retrieval using Reciprocal Rank Fusion.

use crate::rag::hybrid::rrf::{reciprocal_rank_fusion, DEFAULT_RRF_K};
use crate::rag::LlmEmbeddingAdapter;
use async_trait::async_trait;
use mofa_kernel::agent::error::AgentResult;
use mofa_kernel::rag::{Retriever, ScoredDocument};
use mofa_kernel::rag::{HybridRetriever, VectorStore};
use std::sync::Arc;

/// Configuration for the hybrid retriever.
#[derive(Debug, Clone)]
pub struct HybridRetrieverConfig {
    /// Number of results to fetch from each retriever before fusion.
    /// This should be larger than the final top_k to ensure good coverage.
    pub fetch_k: usize,
    /// RRF k parameter (default: 60.0).
    pub rrf_k: f64,
}

impl Default for HybridRetrieverConfig {
    fn default() -> Self {
        Self {
            fetch_k: 20,
            rrf_k: DEFAULT_RRF_K,
        }
    }
}

impl HybridRetrieverConfig {
    /// Create a new config with custom fetch_k.
    pub fn with_fetch_k(mut self, k: usize) -> Self {
        self.fetch_k = k;
        self
    }

    /// Create a new config with custom RRF k parameter.
    pub fn with_rrf_k(mut self, k: f64) -> Self {
        self.rrf_k = k;
        self
    }
}

/// Hybrid retriever that combines dense (vector) and sparse (BM25) retrieval.
///
/// This retriever performs parallel retrieval from both the dense vector store
/// and the sparse BM25 retriever, then combines the results using Reciprocal Rank Fusion.
///
/// # Example
///
/// ```ignore
/// use mofa_foundation::rag::{
///     hybrid::HybridSearchPipeline,
///     InMemoryVectorStore, Bm25Retriever
/// };
///
/// // Create retrievers
/// let dense_store = Arc::new(InMemoryVectorStore::new(384));
/// let sparse_retriever = Arc::new(Bm25Retriever::new());
///
/// // Create hybrid retriever
/// let hybrid = HybridSearchPipeline::new(
///     dense_store,
///     sparse_retriever,
///     embedder,
/// );
///
/// // Retrieve
/// let results = hybrid.retrieve("query", 5).await?;
/// ```
pub struct HybridSearchPipeline {
    /// Dense vector store for semantic search
    dense_store: Arc<dyn VectorStore>,
    /// Sparse BM25 retriever for keyword search
    sparse_retriever: Arc<dyn Retriever>,
    /// Embedding adapter for query embedding
    embedding_adapter: LlmEmbeddingAdapter,
    /// Configuration
    config: HybridRetrieverConfig,
}

impl HybridSearchPipeline {
    /// Create a new hybrid search pipeline.
    pub fn new(
        dense_store: Arc<dyn VectorStore>,
        sparse_retriever: Arc<dyn Retriever>,
        embedding_adapter: LlmEmbeddingAdapter,
    ) -> Self {
        Self {
            dense_store,
            sparse_retriever,
            embedding_adapter,
            config: HybridRetrieverConfig::default(),
        }
    }

    /// Create with custom configuration.
    pub fn with_config(
        dense_store: Arc<dyn VectorStore>,
        sparse_retriever: Arc<dyn Retriever>,
        embedding_adapter: LlmEmbeddingAdapter,
        config: HybridRetrieverConfig,
    ) -> Self {
        Self {
            dense_store,
            sparse_retriever,
            embedding_adapter,
            config,
        }
    }

    /// Retrieve from dense vector store.
    async fn retrieve_dense(&self, query: &str, top_k: usize) -> AgentResult<Vec<ScoredDocument>> {
        // Embed the query
        let query_embedding = self.embedding_adapter
            .embed_one(query)
            .await
            .map_err(|e| mofa_kernel::agent::error::AgentError::ExecutionFailed(e.to_string()))?;

        // Search the vector store
        let search_results = self
            .dense_store
            .search(&query_embedding, top_k, None)
            .await?;

        // Convert to ScoredDocument
        let docs: Vec<ScoredDocument> = search_results
            .into_iter()
            .map(|result| {
                ScoredDocument::new(
                    mofa_kernel::rag::Document::new(result.id.clone(), result.text.clone()),
                    result.score,
                    Some("dense".to_string()),
                )
            })
            .collect();

        Ok(docs)
    }

    /// Retrieve from sparse BM25 retriever.
    async fn retrieve_sparse(&self, query: &str, top_k: usize) -> AgentResult<Vec<ScoredDocument>> {
        self.sparse_retriever.retrieve(query, top_k).await
    }
}

#[async_trait]
impl HybridRetriever for HybridSearchPipeline {
    async fn retrieve(&self, query: &str, top_k: usize) -> AgentResult<Vec<ScoredDocument>> {
        self.retrieve_with_rrf(query, top_k, DEFAULT_RRF_K).await
    }

    async fn retrieve_with_rrf(
        &self,
        query: &str,
        top_k: usize,
        rrf_k: f64,
    ) -> AgentResult<Vec<ScoredDocument>> {
        if query.trim().is_empty() {
            return Ok(Vec::new());
        }

        // Fetch more results from each retriever to ensure good coverage
        let fetch_k = self.config.fetch_k.max(top_k);

        // Parallel retrieval from both sources
        let (dense_results, sparse_results) = tokio::join!(
            self.retrieve_dense(query, fetch_k),
            self.retrieve_sparse(query, fetch_k)
        );

        let dense = dense_results.unwrap_or_default();
        let sparse = sparse_results.unwrap_or_default();

        // Apply RRF fusion
        let fused = reciprocal_rank_fusion(&[dense, sparse], rrf_k, top_k);

        Ok(fused)
    }
}

/// A simpler hybrid retriever that works with any Retriever implementations.
///
/// This is useful when you already have retriever implementations and want
/// to combine them without needing an embedding adapter.
pub struct GenericHybridRetriever {
    /// First retriever (typically dense)
    retriever_a: Arc<dyn Retriever>,
    /// Second retriever (typically sparse)
    retriever_b: Arc<dyn Retriever>,
    /// Configuration
    config: HybridRetrieverConfig,
}

impl GenericHybridRetriever {
    /// Create a new generic hybrid retriever.
    pub fn new(retriever_a: Arc<dyn Retriever>, retriever_b: Arc<dyn Retriever>) -> Self {
        Self {
            retriever_a,
            retriever_b,
            config: HybridRetrieverConfig::default(),
        }
    }

    /// Create with custom configuration.
    pub fn with_config(
        retriever_a: Arc<dyn Retriever>,
        retriever_b: Arc<dyn Retriever>,
        config: HybridRetrieverConfig,
    ) -> Self {
        Self {
            retriever_a,
            retriever_b,
            config,
        }
    }
}

#[async_trait]
impl HybridRetriever for GenericHybridRetriever {
    async fn retrieve(&self, query: &str, top_k: usize) -> AgentResult<Vec<ScoredDocument>> {
        self.retrieve_with_rrf(query, top_k, DEFAULT_RRF_K).await
    }

    async fn retrieve_with_rrf(
        &self,
        query: &str,
        top_k: usize,
        rrf_k: f64,
    ) -> AgentResult<Vec<ScoredDocument>> {
        if query.trim().is_empty() {
            return Ok(Vec::new());
        }

        let fetch_k = self.config.fetch_k.max(top_k);

        // Parallel retrieval
        let (results_a, results_b) = tokio::join!(
            self.retriever_a.retrieve(query, fetch_k),
            self.retriever_b.retrieve(query, fetch_k)
        );

        let a = results_a.unwrap_or_default();
        let b = results_b.unwrap_or_default();

        // Apply RRF fusion
        let fused = reciprocal_rank_fusion(&[a, b], rrf_k, top_k);

        Ok(fused)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mofa_kernel::rag::Document;
    use mofa_kernel::rag::DocumentChunk;
    use mofa_kernel::rag::SimilarityMetric;
    use std::collections::HashMap;

    // Mock vector store
    struct MockVectorStore {
        chunks: HashMap<String, DocumentChunk>,
    }

    impl MockVectorStore {
        fn new() -> Self {
            Self {
                chunks: HashMap::new(),
            }
        }
    }

    #[async_trait]
    impl VectorStore for MockVectorStore {
        async fn upsert(&mut self, chunk: DocumentChunk) -> AgentResult<()> {
            self.chunks.insert(chunk.id.clone(), chunk);
            Ok(())
        }

        async fn search(
            &self,
            _query_embedding: &[f32],
            top_k: usize,
            _threshold: Option<f32>,
        ) -> AgentResult<Vec<mofa_kernel::rag::SearchResult>> {
            let results: Vec<mofa_kernel::rag::SearchResult> = self
                .chunks
                .values()
                .take(top_k)
                .map(|c| mofa_kernel::rag::SearchResult {
                    id: c.id.clone(),
                    text: c.text.clone(),
                    score: 0.9,
                    metadata: c.metadata.clone(),
                })
                .collect();
            Ok(results)
        }

        async fn delete(&mut self, _id: &str) -> AgentResult<bool> {
            Ok(false)
        }

        async fn clear(&mut self) -> AgentResult<()> {
            self.chunks.clear();
            Ok(())
        }

        async fn count(&self) -> AgentResult<usize> {
            Ok(self.chunks.len())
        }

        fn similarity_metric(&self) -> SimilarityMetric {
            SimilarityMetric::Cosine
        }
    }

    // Mock retriever for sparse
    struct MockSparseRetriever {
        docs: Vec<ScoredDocument>,
    }

    #[async_trait]
    impl Retriever for MockSparseRetriever {
        async fn retrieve(&self, _query: &str, top_k: usize) -> AgentResult<Vec<ScoredDocument>> {
            Ok(self.docs.iter().take(top_k).cloned().collect())
        }
    }

    #[tokio::test]
    async fn test_generic_hybrid_retriever() {
        let retriever_a = Arc::new(MockSparseRetriever {
            docs: vec![
                ScoredDocument::new(Document::new("a1", "doc a1"), 0.9, Some("a".to_string())),
                ScoredDocument::new(Document::new("a2", "doc a2"), 0.8, Some("a".to_string())),
            ],
        }) as Arc<dyn Retriever>;

        let retriever_b = Arc::new(MockSparseRetriever {
            docs: vec![
                ScoredDocument::new(Document::new("b1", "doc b1"), 0.95, Some("b".to_string())),
                ScoredDocument::new(Document::new("b2", "doc b2"), 0.85, Some("b".to_string())),
            ],
        }) as Arc<dyn Retriever>;

        let hybrid = GenericHybridRetriever::new(retriever_a, retriever_b);
        let results = hybrid.retrieve("test", 3).await.unwrap();

        assert!(!results.is_empty());
    }

    #[tokio::test]
    async fn test_config_options() {
        let config = HybridRetrieverConfig::default()
            .with_fetch_k(50)
            .with_rrf_k(30.0);

        assert_eq!(config.fetch_k, 50);
        assert_eq!(config.rrf_k, 30.0);
    }
}
