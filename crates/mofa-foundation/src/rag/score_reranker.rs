//! Score-based reranker with threshold filtering
//!
//! Provides a configurable reranker that filters results by minimum score
//! threshold and optionally limits the number of returned results.

use async_trait::async_trait;
use mofa_kernel::agent::error::AgentResult;
use mofa_kernel::rag::{Reranker, ScoredDocument};

// =============================================================================
// ScoreReranker
// =============================================================================

/// Score-based reranker with threshold and top-k filtering.
///
/// Filters documents by minimum relevance score and returns the top-k
/// most relevant results sorted by score (descending).
#[derive(Debug, Clone)]
pub struct ScoreReranker {
    /// Minimum score threshold (documents below this are dropped)
    pub min_score: f32,
    /// Maximum number of documents to return (None = no limit)
    pub max_results: Option<usize>,
}

impl Default for ScoreReranker {
    fn default() -> Self {
        Self {
            min_score: 0.0,
            max_results: None,
        }
    }
}

impl ScoreReranker {
    /// Create a reranker with the given minimum score threshold.
    #[must_use]
    pub fn with_threshold(min_score: f32) -> Self {
        Self {
            min_score,
            max_results: None,
        }
    }

    /// Create a reranker with threshold and top-k limit.
    #[must_use]
    pub fn new(min_score: f32, max_results: usize) -> Self {
        Self {
            min_score,
            max_results: Some(max_results),
        }
    }
}

#[async_trait]
impl Reranker for ScoreReranker {
    async fn rerank(
        &self,
        _query: &str,
        mut docs: Vec<ScoredDocument>,
    ) -> AgentResult<Vec<ScoredDocument>> {
        // Filter by minimum score
        docs.retain(|d| d.score >= self.min_score);

        // Sort by score descending, with deterministic tie-breaker on document.id
        docs.sort_by(|a, b| {
            match b.score.partial_cmp(&a.score) {
                Some(ordering) if ordering != std::cmp::Ordering::Equal => ordering,
                _ => a.document.id.cmp(&b.document.id),
            }
        });

        // Apply top-k limit
        if let Some(max) = self.max_results {
            docs.truncate(max);
        }

        Ok(docs)
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use mofa_kernel::rag::{Document, ScoredDocument};

    fn make_doc(id: &str, score: f32) -> ScoredDocument {
        ScoredDocument::new(Document::new(id, format!("content of {id}")), score, None)
    }

    #[tokio::test]
    async fn filters_by_threshold() {
        let reranker = ScoreReranker::with_threshold(0.5);
        let docs = vec![
            make_doc("a", 0.9),
            make_doc("b", 0.3),
            make_doc("c", 0.7),
            make_doc("d", 0.1),
        ];
        let result = reranker.rerank("query", docs).await.unwrap();
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].document.id, "a");
        assert_eq!(result[1].document.id, "c");
    }

    #[tokio::test]
    async fn sorts_by_score_descending() {
        let reranker = ScoreReranker::default();
        let docs = vec![
            make_doc("a", 0.3),
            make_doc("b", 0.9),
            make_doc("c", 0.6),
        ];
        let result = reranker.rerank("query", docs).await.unwrap();
        assert_eq!(result[0].document.id, "b");
        assert_eq!(result[1].document.id, "c");
        assert_eq!(result[2].document.id, "a");
    }

    #[tokio::test]
    async fn limits_top_k() {
        let reranker = ScoreReranker::new(0.0, 2);
        let docs = vec![
            make_doc("a", 0.9),
            make_doc("b", 0.7),
            make_doc("c", 0.5),
            make_doc("d", 0.3),
        ];
        let result = reranker.rerank("query", docs).await.unwrap();
        assert_eq!(result.len(), 2);
    }

    #[tokio::test]
    async fn threshold_and_top_k_combined() {
        let reranker = ScoreReranker::new(0.4, 2);
        let docs = vec![
            make_doc("a", 0.9),
            make_doc("b", 0.7),
            make_doc("c", 0.5),
            make_doc("d", 0.3),
        ];
        let result = reranker.rerank("query", docs).await.unwrap();
        // d is filtered by threshold (0.3 < 0.4), then sorted to [a, b, c] and top-2 are taken
        assert_eq!(result.len(), 2);
        assert!(result.iter().all(|d| d.score >= 0.4));
    }

    #[tokio::test]
    async fn empty_input() {
        let reranker = ScoreReranker::default();
        let result = reranker.rerank("query", vec![]).await.unwrap();
        assert!(result.is_empty());
    }

    #[tokio::test]
    async fn all_filtered_out() {
        let reranker = ScoreReranker::with_threshold(0.99);
        let docs = vec![
            make_doc("a", 0.5),
            make_doc("b", 0.3),
        ];
        let result = reranker.rerank("query", docs).await.unwrap();
        assert!(result.is_empty());
    }
}
