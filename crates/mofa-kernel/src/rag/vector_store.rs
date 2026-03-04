//! VectorStore trait definition
//!
//! Defines the abstract interface for vector storage and similarity search.
//! Concrete implementations (InMemoryVectorStore, etc.) live in mofa-foundation.

use crate::agent::error::AgentResult;
use crate::rag::types::{DocumentChunk, SearchResult, SimilarityMetric};
use async_trait::async_trait;

/// Abstract interface for vector storage and similarity search.
///
/// Implementations of this trait provide the ability to store embedding vectors
/// and retrieve the most similar ones given a query vector. This is the core
/// building block for RAG (Retrieval-Augmented Generation) pipelines.
///
/// # Example
///
/// ```rust,ignore
/// use mofa_kernel::rag::{VectorStore, DocumentChunk, SimilarityMetric};
///
/// // Store a document chunk
/// let chunk = DocumentChunk::new("id-1", "MoFA is a Rust agent framework", embedding);
/// store.upsert(chunk).await?;
///
/// // Search for similar chunks
/// let results = store.search(&query_embedding, 5, None).await?;
/// for result in results {
///     println!("{}: {}", result.score, result.text);
/// }
/// ```
#[async_trait]
pub trait VectorStore: Send + Sync {
    /// Insert or update a document chunk in the store.
    ///
    /// If a chunk with the same id already exists, it will be replaced.
    async fn upsert(&mut self, chunk: DocumentChunk) -> AgentResult<()>;

    /// Insert or update multiple document chunks at once.
    async fn upsert_batch(&mut self, chunks: Vec<DocumentChunk>) -> AgentResult<()> {
        for chunk in chunks {
            self.upsert(chunk).await?;
        }
        Ok(())
    }

    /// Search for the most similar chunks to the given query embedding.
    ///
    /// Returns up to `top_k` results sorted by similarity score (highest first).
    /// If `threshold` is provided, only results with a score above the threshold
    /// are returned.
    async fn search(
        &self,
        query_embedding: &[f32],
        top_k: usize,
        threshold: Option<f32>,
    ) -> AgentResult<Vec<SearchResult>>;

    /// Delete a chunk by its id.
    ///
    /// Returns true if the chunk existed and was deleted, false otherwise.
    async fn delete(&mut self, id: &str) -> AgentResult<bool>;

    /// Remove all chunks from the store.
    async fn clear(&mut self) -> AgentResult<()>;

    /// Get the total number of chunks in the store.
    async fn count(&self) -> AgentResult<usize>;

    /// Get the similarity metric used by this store.
    fn similarity_metric(&self) -> SimilarityMetric;
}
