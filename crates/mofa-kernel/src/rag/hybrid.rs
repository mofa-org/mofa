//! Hybrid Retriever trait definition
//!
//! Defines the interface for hybrid retrieval combining dense vector search
//! with BM25 sparse retrieval using Reciprocal Rank Fusion (RRF).

use crate::agent::error::AgentResult;
use crate::rag::types::ScoredDocument;
use async_trait::async_trait;

/// Hybrid retriever that combines dense (vector) and sparse (BM25) retrieval.
///
/// This trait extends the basic [`Retriever`] capability to support hybrid
/// retrieval pipelines that combine semantic similarity (dense embeddings)
/// with keyword matching (BM25) for improved recall.
///
/// The default RRF parameter of 60 is commonly used in production systems
/// and provides a good balance between the two retrieval methods.
#[async_trait]
pub trait HybridRetriever: Send + Sync {
    /// Retrieve documents using hybrid dense + sparse retrieval.
    ///
    /// This method performs parallel retrieval from both dense and sparse
    /// retrievers, then combines the results using Reciprocal Rank Fusion.
    ///
    /// # Arguments
    /// * `query` - The search query string
    /// * `top_k` - Maximum number of results to return
    ///
    /// # Returns
    /// A vector of scored documents sorted by their fused RRF scores.
    async fn retrieve(&self, query: &str, top_k: usize) -> AgentResult<Vec<ScoredDocument>>;

    /// Retrieve with custom RRF parameter.
    ///
    /// Allows fine-tuning the fusion parameter for specific use cases.
    /// Higher values give more weight to lower-ranked results.
    ///
    /// # Arguments
    /// * `query` - The search query string
    /// * `top_k` - Maximum number of results to return
    /// * `rrf_k` - RRF k parameter (typically 60)
    async fn retrieve_with_rrf(
        &self,
        query: &str,
        top_k: usize,
        rrf_k: f64,
    ) -> AgentResult<Vec<ScoredDocument>>;
}
