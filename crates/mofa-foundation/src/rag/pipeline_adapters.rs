//! In-memory RAG pipeline adapters
//!
//! Concrete foundation-level implementations of kernel RAG contracts:
//! - InMemoryRetriever
//! - IdentityReranker
//! - SimpleGenerator

use async_trait::async_trait;
use mofa_kernel::agent::error::{AgentError, AgentResult};
use mofa_kernel::rag::{Document, GenerateInput, Generator, Reranker, Retriever, ScoredDocument};

/// Simple in-memory retriever over a static corpus of search results.
///
/// Retrieval score is computed from token overlap with query text,
/// then combined with the base score in each corpus item.
pub struct InMemoryRetriever {
    corpus: Vec<ScoredDocument>,
}

impl InMemoryRetriever {
    /// Create a retriever from pre-indexed in-memory entries.
    pub fn new(corpus: Vec<ScoredDocument>) -> Self {
        Self { corpus }
    }

    /// Return current corpus size.
    pub fn len(&self) -> usize {
        self.corpus.len()
    }

    /// Whether corpus is empty.
    pub fn is_empty(&self) -> bool {
        self.corpus.is_empty()
    }
}

#[async_trait]
impl Retriever for InMemoryRetriever {
    async fn retrieve(&self, query: &str, top_k: usize) -> AgentResult<Vec<ScoredDocument>> {
        if top_k == 0 {
            return Err(AgentError::InvalidInput(
                "top_k must be greater than 0".to_string(),
            ));
        }

        let query_terms = tokenize(query);

        let mut scored = self
            .corpus
            .iter()
            .map(|item| {
                let text_terms = tokenize(&item.document.text);
                let overlap = text_terms.intersection(&query_terms).count() as f32;
                let lexical = if query_terms.is_empty() {
                    0.0
                } else {
                    overlap / query_terms.len() as f32
                };

                let mut ranked = item.clone();
                ranked.score = ranked.score + lexical;
                ranked
            })
            .collect::<Vec<_>>();

        scored.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| a.document.id.cmp(&b.document.id))
        });
        scored.truncate(top_k);

        Ok(scored)
    }
}

/// Reranker that preserves incoming order and scores.
pub struct IdentityReranker;

#[async_trait]
impl Reranker for IdentityReranker {
    async fn rerank(
        &self,
        _query: &str,
        docs: Vec<ScoredDocument>,
    ) -> AgentResult<Vec<ScoredDocument>> {
        Ok(docs)
    }
}

/// Deterministic generator for local development/testing.
///
/// Builds an answer by concatenating up to `max_contexts` snippets.
pub struct SimpleGenerator {
    max_contexts: usize,
}

impl SimpleGenerator {
    /// Create generator with default max context count.
    pub fn new() -> Self {
        Self { max_contexts: 3 }
    }

    /// Configure max number of context chunks included in final answer.
    pub fn with_max_contexts(max_contexts: usize) -> Self {
        Self { max_contexts }
    }
}

impl Default for SimpleGenerator {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Generator for SimpleGenerator {
    async fn generate(&self, input: &GenerateInput) -> AgentResult<String> {
        if input.context.is_empty() {
            return Ok(format!(
                "No relevant context found. Question: {}",
                input.query
            ));
        }

        let snippets = input
            .context
            .iter()
            .take(self.max_contexts)
            .map(|document| format!("[{}] {}", document.id, document.text))
            .collect::<Vec<_>>()
            .join(" | ");

        Ok(format!("Answer for '{}': {}", input.query, snippets))
    }
}

fn tokenize(input: &str) -> std::collections::HashSet<String> {
    input
        .split(|character: char| !character.is_alphanumeric())
        .filter(|token| !token.is_empty())
        .map(|token| token.to_ascii_lowercase())
        .collect::<std::collections::HashSet<_>>()
}

#[cfg(test)]
mod tests {
    use super::*;
    use mofa_kernel::agent::error::{AgentError, AgentResult};
    use mofa_kernel::rag::RagPipeline;
    use std::sync::Arc;

    fn scored(id: &str, text: &str, score: f32) -> ScoredDocument {
        ScoredDocument::new(Document::new(id, text), score, Some("memory".to_string()))
    }

    fn sample_corpus() -> Vec<ScoredDocument> {
        vec![
            scored(
                "doc-rust",
                "MoFA is a Rust-native microkernel framework",
                0.50,
            ),
            scored(
                "doc-python",
                "Python agents can integrate through FFI",
                0.35,
            ),
            scored("doc-rag", "RAG combines retrieval with generation", 0.45),
        ]
    }

    #[tokio::test]
    async fn retrieve_rerank_generate_end_to_end() {
        let pipeline = RagPipeline::new(
            Arc::new(InMemoryRetriever::new(sample_corpus())),
            Arc::new(IdentityReranker),
            Arc::new(SimpleGenerator::new()),
        );

        let result = pipeline
            .run_with_top_k("How does MoFA use Rust for RAG?", 2)
            .await
            .unwrap();

        assert_eq!(result.retrieved_docs.len(), 2);
        assert_eq!(result.reranked_docs.len(), 2);
        assert!(result.answer.contains("Answer for"));
        assert!(
            result.answer.contains("doc-rust")
                || result.answer.contains("doc-rag")
                || result.answer.contains("doc-python")
        );
    }

    #[tokio::test]
    async fn top_k_zero_remains_validation_error() {
        let pipeline = RagPipeline::new(
            Arc::new(InMemoryRetriever::new(sample_corpus())),
            Arc::new(IdentityReranker),
            Arc::new(SimpleGenerator::default()),
        );

        let err = pipeline
            .run_with_top_k("invalid", 0)
            .await
            .expect_err("top_k=0 must fail");

        assert!(matches!(err, AgentError::InvalidInput(_)));
    }

    #[tokio::test]
    async fn empty_retrieval_still_generates_fallback() {
        let pipeline = RagPipeline::new(
            Arc::new(InMemoryRetriever::new(vec![])),
            Arc::new(IdentityReranker),
            Arc::new(SimpleGenerator::new()),
        );

        let result = pipeline
            .run_with_top_k("What is retrieved?", 3)
            .await
            .unwrap();

        assert!(result.retrieved_docs.is_empty());
        assert!(result.answer.starts_with("No relevant context found"));
    }

    struct FailingRetriever;

    #[async_trait]
    impl Retriever for FailingRetriever {
        async fn retrieve(&self, _query: &str, _top_k: usize) -> AgentResult<Vec<ScoredDocument>> {
            Err(AgentError::ExecutionFailed("retriever boom".to_string()))
        }
    }

    #[tokio::test]
    async fn component_errors_propagate_unchanged() {
        let pipeline = RagPipeline::new(
            Arc::new(FailingRetriever),
            Arc::new(IdentityReranker),
            Arc::new(SimpleGenerator::new()),
        );

        let err = pipeline
            .run_with_top_k("trigger", 2)
            .await
            .expect_err("retriever error should propagate");

        match err {
            AgentError::ExecutionFailed(message) => assert!(message.contains("retriever boom")),
            _ => panic!("unexpected error type"),
        }
    }

    #[tokio::test]
    async fn deterministic_ordering_for_stable_tests() {
        let retriever = InMemoryRetriever::new(vec![
            scored("b", "rust retrieval", 1.0),
            scored("a", "rust retrieval", 1.0),
        ]);

        let ranked = retriever.retrieve("rust", 2).await.unwrap();

        assert_eq!(ranked[0].document.id, "a");
        assert_eq!(ranked[1].document.id, "b");
    }
}
