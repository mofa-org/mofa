//! Default RAG implementations

use async_trait::async_trait;
use mofa_kernel::agent::error::AgentResult;
use mofa_kernel::rag::pipeline::Reranker;
use mofa_kernel::rag::ScoredDocument;

/// Identity reranker that returns documents in the same order
pub struct IdentityReranker;

#[async_trait]
impl Reranker for IdentityReranker {
    async fn rerank(&self, _query: &str, docs: Vec<ScoredDocument>) -> AgentResult<Vec<ScoredDocument>> {
        Ok(docs)
    }
}