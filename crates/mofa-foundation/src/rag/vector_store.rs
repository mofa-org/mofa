//! In-memory vector store implementation
//!
//! Provides a simple brute-force vector store backed by a HashMap.
//! Suitable for development, testing, and small datasets.

use crate::rag::similarity::compute_similarity;
use async_trait::async_trait;
use mofa_kernel::agent::error::AgentResult;
use mofa_kernel::rag::{DocumentChunk, SearchResult, SimilarityMetric, VectorStore};
use std::collections::HashMap;

/// In-memory vector store using brute-force similarity search.
///
/// Stores all document chunks in a HashMap and computes similarity
/// against every stored vector on each search. This is simple and
/// works well for small to medium datasets (up to ~10k chunks).
///
/// # Example
///
/// ```rust,ignore
/// use mofa_foundation::rag::{InMemoryVectorStore, SimilarityMetric, DocumentChunk, VectorStore};
///
/// let mut store = InMemoryVectorStore::new(SimilarityMetric::Cosine);
///
/// let chunk = DocumentChunk::new("doc-1", "Hello world", vec![0.1, 0.2, 0.3]);
/// store.upsert(chunk).await?;
///
/// let results = store.search(&[0.1, 0.2, 0.3], 5, None).await?;
/// ```
pub struct InMemoryVectorStore {
    chunks: HashMap<String, DocumentChunk>,
    metric: SimilarityMetric,
}

impl InMemoryVectorStore {
    /// Create a new empty in-memory vector store with the given similarity metric.
    pub fn new(metric: SimilarityMetric) -> Self {
        Self {
            chunks: HashMap::new(),
            metric,
        }
    }

    /// Create a new store using cosine similarity (the most common default).
    pub fn cosine() -> Self {
        Self::new(SimilarityMetric::Cosine)
    }
}

impl Default for InMemoryVectorStore {
    fn default() -> Self {
        Self::cosine()
    }
}

#[async_trait]
impl VectorStore for InMemoryVectorStore {
    async fn upsert(&mut self, chunk: DocumentChunk) -> AgentResult<()> {
        self.chunks.insert(chunk.id.clone(), chunk);
        Ok(())
    }

    async fn upsert_batch(&mut self, chunks: Vec<DocumentChunk>) -> AgentResult<()> {
        for chunk in chunks {
            self.chunks.insert(chunk.id.clone(), chunk);
        }
        Ok(())
    }

    async fn search(
        &self,
        query_embedding: &[f32],
        top_k: usize,
        threshold: Option<f32>,
    ) -> AgentResult<Vec<SearchResult>> {
        let mut scored: Vec<SearchResult> = self
            .chunks
            .values()
            .map(|chunk| {
                let score = compute_similarity(&chunk.embedding, query_embedding, self.metric);
                SearchResult::from_chunk(chunk, score)
            })
            .filter(|result| {
                if let Some(t) = threshold {
                    result.score >= t
                } else {
                    true
                }
            })
            .collect();

        scored.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        scored.truncate(top_k);

        Ok(scored)
    }

    async fn delete(&mut self, id: &str) -> AgentResult<bool> {
        Ok(self.chunks.remove(id).is_some())
    }

    async fn clear(&mut self) -> AgentResult<()> {
        self.chunks.clear();
        Ok(())
    }

    async fn count(&self) -> AgentResult<usize> {
        Ok(self.chunks.len())
    }

    fn similarity_metric(&self) -> SimilarityMetric {
        self.metric
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_chunk(id: &str, text: &str, embedding: Vec<f32>) -> DocumentChunk {
        DocumentChunk::new(id, text, embedding)
    }

    #[tokio::test]
    async fn test_upsert_and_count() {
        let mut store = InMemoryVectorStore::cosine();
        assert_eq!(store.count().await.unwrap(), 0);

        store
            .upsert(make_chunk("1", "hello", vec![1.0, 0.0, 0.0]))
            .await
            .unwrap();
        assert_eq!(store.count().await.unwrap(), 1);

        store
            .upsert(make_chunk("2", "world", vec![0.0, 1.0, 0.0]))
            .await
            .unwrap();
        assert_eq!(store.count().await.unwrap(), 2);
    }

    #[tokio::test]
    async fn test_upsert_replaces_existing() {
        let mut store = InMemoryVectorStore::cosine();

        store
            .upsert(make_chunk("1", "old text", vec![1.0, 0.0]))
            .await
            .unwrap();
        store
            .upsert(make_chunk("1", "new text", vec![0.0, 1.0]))
            .await
            .unwrap();

        assert_eq!(store.count().await.unwrap(), 1);

        let results = store.search(&[0.0, 1.0], 1, None).await.unwrap();
        assert_eq!(results[0].text, "new text");
    }

    #[tokio::test]
    async fn test_upsert_batch() {
        let mut store = InMemoryVectorStore::cosine();

        let chunks = vec![
            make_chunk("1", "one", vec![1.0, 0.0]),
            make_chunk("2", "two", vec![0.0, 1.0]),
            make_chunk("3", "three", vec![1.0, 1.0]),
        ];
        store.upsert_batch(chunks).await.unwrap();
        assert_eq!(store.count().await.unwrap(), 3);
    }

    #[tokio::test]
    async fn test_search_returns_most_similar() {
        let mut store = InMemoryVectorStore::cosine();

        store
            .upsert(make_chunk("a", "rust lang", vec![1.0, 0.0, 0.0]))
            .await
            .unwrap();
        store
            .upsert(make_chunk("b", "python lang", vec![0.0, 1.0, 0.0]))
            .await
            .unwrap();
        store
            .upsert(make_chunk("c", "mostly rust", vec![0.9, 0.1, 0.0]))
            .await
            .unwrap();

        let results = store.search(&[1.0, 0.0, 0.0], 2, None).await.unwrap();

        assert_eq!(results.len(), 2);
        assert_eq!(results[0].id, "a");
        assert_eq!(results[1].id, "c");
    }

    #[tokio::test]
    async fn test_search_with_threshold() {
        let mut store = InMemoryVectorStore::cosine();

        store
            .upsert(make_chunk("close", "close match", vec![1.0, 0.0]))
            .await
            .unwrap();
        store
            .upsert(make_chunk("far", "far away", vec![0.0, 1.0]))
            .await
            .unwrap();

        let results = store.search(&[1.0, 0.0], 10, Some(0.9)).await.unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].id, "close");
    }

    #[tokio::test]
    async fn test_delete() {
        let mut store = InMemoryVectorStore::cosine();

        store
            .upsert(make_chunk("1", "hello", vec![1.0, 0.0]))
            .await
            .unwrap();
        assert_eq!(store.count().await.unwrap(), 1);

        let deleted = store.delete("1").await.unwrap();
        assert!(deleted);
        assert_eq!(store.count().await.unwrap(), 0);

        let deleted_again = store.delete("1").await.unwrap();
        assert!(!deleted_again);
    }

    #[tokio::test]
    async fn test_clear() {
        let mut store = InMemoryVectorStore::cosine();

        store.upsert(make_chunk("1", "a", vec![1.0])).await.unwrap();
        store.upsert(make_chunk("2", "b", vec![2.0])).await.unwrap();
        assert_eq!(store.count().await.unwrap(), 2);

        store.clear().await.unwrap();
        assert_eq!(store.count().await.unwrap(), 0);
    }

    #[tokio::test]
    async fn test_euclidean_metric() {
        let mut store = InMemoryVectorStore::new(SimilarityMetric::Euclidean);

        store
            .upsert(make_chunk("near", "near", vec![1.0, 0.0]))
            .await
            .unwrap();
        store
            .upsert(make_chunk("far", "far", vec![10.0, 10.0]))
            .await
            .unwrap();

        let results = store.search(&[1.0, 0.0], 2, None).await.unwrap();
        assert_eq!(results[0].id, "near");
    }

    #[tokio::test]
    async fn test_dot_product_metric() {
        let mut store = InMemoryVectorStore::new(SimilarityMetric::DotProduct);

        store
            .upsert(make_chunk("big", "big projection", vec![2.0, 3.0]))
            .await
            .unwrap();
        store
            .upsert(make_chunk("small", "small projection", vec![0.1, 0.1]))
            .await
            .unwrap();

        let results = store.search(&[1.0, 1.0], 2, None).await.unwrap();
        assert_eq!(results[0].id, "big");
    }

    #[test]
    fn test_default_is_cosine() {
        let store = InMemoryVectorStore::default();
        assert_eq!(store.similarity_metric(), SimilarityMetric::Cosine);
    }
}
