//! RAG pipeline contracts and orchestration.

use crate::agent::error::{AgentError, AgentResult};
use crate::rag::types::{GenerateInput, ScoredDocument};
use async_trait::async_trait;
use futures::stream::Stream;
use std::pin::Pin;
use std::sync::Arc;

/// A chunk of generated content from a streaming generator.
#[derive(Debug, Clone)]
pub enum GeneratorChunk {
    /// A piece of text content
    Text(String),
    /// End of stream marker
    Done,
}

#[async_trait]
pub trait Retriever: Send + Sync {
    async fn retrieve(&self, query: &str, top_k: usize) -> AgentResult<Vec<ScoredDocument>>;
}

#[async_trait]
pub trait Reranker: Send + Sync {
    async fn rerank(&self, query: &str, docs: Vec<ScoredDocument>) -> AgentResult<Vec<ScoredDocument>>;
}

#[async_trait]
pub trait Generator: Send + Sync {
    async fn generate(&self, input: &GenerateInput) -> AgentResult<String>;

    /// Stream generation results as they become available.
    /// Default implementation falls back to generate() and yields the result as a single chunk.
    async fn stream(
        &self,
        input: GenerateInput,
    ) -> AgentResult<Pin<Box<dyn Stream<Item = AgentResult<GeneratorChunk>> + Send>>> {
        let result = self.generate(&input).await?;
        let stream = futures::stream::once(async move { Ok(GeneratorChunk::Text(result)) });
        Ok(Box::pin(stream))
    }
}

#[derive(Debug, Clone)]
pub struct RagPipelineOutput {
    pub answer: String,
    pub retrieved_docs: Vec<ScoredDocument>,
    pub reranked_docs: Vec<ScoredDocument>,
}

#[derive(Clone)]
pub struct RagPipeline {
    retriever: Arc<dyn Retriever>,
    reranker: Arc<dyn Reranker>,
    generator: Arc<dyn Generator>,
    default_top_k: usize,
}

impl RagPipeline {
    pub fn new(
        retriever: Arc<dyn Retriever>,
        reranker: Arc<dyn Reranker>,
        generator: Arc<dyn Generator>,
    ) -> Self {
        Self {
            retriever,
            reranker,
            generator,
            default_top_k: 5,
        }
    }

    pub fn with_default_top_k(mut self, top_k: usize) -> Self {
        self.default_top_k = top_k;
        self
    }

    pub async fn run(&self, query: &str) -> AgentResult<RagPipelineOutput> {
        self.run_with_top_k(query, self.default_top_k).await
    }

    pub async fn run_with_top_k(&self, query: &str, top_k: usize) -> AgentResult<RagPipelineOutput> {
        if top_k == 0 {
            return Err(AgentError::InvalidInput("top_k must be greater than 0".to_string()));
        }

        let retrieved_docs = self.retriever.retrieve(query, top_k).await?;
        let reranked_docs = self.reranker.rerank(query, retrieved_docs.clone()).await?;

        let context = reranked_docs
            .iter()
            .map(|doc| doc.document.clone())
            .collect();

        let generate_input = GenerateInput::new(query, context);
        let answer = self.generator.generate(&generate_input).await?;

        Ok(RagPipelineOutput {
            answer,
            retrieved_docs,
            reranked_docs,
        })
    }

    pub async fn run_streaming(
        &self,
        query: &str,
        top_k: usize,
    ) -> AgentResult<(
        Vec<ScoredDocument>,
        Pin<Box<dyn Stream<Item = AgentResult<GeneratorChunk>> + Send>>,
    )> {
        if top_k == 0 {
            return Err(AgentError::InvalidInput("top_k must be greater than 0".to_string()));
        }

        let retrieved_docs = self.retriever.retrieve(query, top_k).await?;
        let reranked_docs = self.reranker.rerank(query, retrieved_docs.clone()).await?;

        let context = reranked_docs
            .iter()
            .map(|doc| doc.document.clone())
            .collect();

        let generate_input = GenerateInput::new(query, context);
        let stream = self.generator.stream(generate_input).await?;

        Ok((reranked_docs, stream))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rag::types::Document;
    use std::sync::{Arc, Mutex};

    struct FakeRetriever {
        docs: Vec<ScoredDocument>,
        last_top_k: Arc<Mutex<Option<usize>>>,
    }

    impl FakeRetriever {
        fn new(docs: Vec<ScoredDocument>) -> Self {
            Self {
                docs,
                last_top_k: Arc::new(Mutex::new(None)),
            }
        }
    }

    #[async_trait]
    impl Retriever for FakeRetriever {
        async fn retrieve(&self, _query: &str, top_k: usize) -> AgentResult<Vec<ScoredDocument>> {
            let mut guard = self.last_top_k.lock().unwrap();
            *guard = Some(top_k);
            Ok(self.docs.iter().take(top_k).cloned().collect())
        }
    }

    struct IdentityReranker;

    #[async_trait]
    impl Reranker for IdentityReranker {
        async fn rerank(&self, _query: &str, docs: Vec<ScoredDocument>) -> AgentResult<Vec<ScoredDocument>> {
            Ok(docs)
        }
    }

    struct ReverseReranker;

    #[async_trait]
    impl Reranker for ReverseReranker {
        async fn rerank(&self, _query: &str, mut docs: Vec<ScoredDocument>) -> AgentResult<Vec<ScoredDocument>> {
            docs.reverse();
            Ok(docs)
        }
    }

    struct FakeGenerator;

    #[async_trait]
    impl Generator for FakeGenerator {
        async fn generate(&self, input: &GenerateInput) -> AgentResult<String> {
            Ok(format!("Q: {} | ctx={}", input.query, input.context.len()))
        }
    }

    struct FailingRetriever;

    #[async_trait]
    impl Retriever for FailingRetriever {
        async fn retrieve(&self, _query: &str, _top_k: usize) -> AgentResult<Vec<ScoredDocument>> {
            Err(AgentError::ExecutionFailed("retrieval failed".to_string()))
        }
    }

    fn scored(id: &str, text: &str, score: f32) -> ScoredDocument {
        ScoredDocument::new(Document::new(id, text), score, Some("sparse".to_string()))
    }

    #[tokio::test]
    async fn pipeline_happy_path() {
        let retriever = Arc::new(FakeRetriever::new(vec![scored("1", "a", 0.9)]));
        let reranker = Arc::new(IdentityReranker);
        let generator = Arc::new(FakeGenerator);
        let pipeline = RagPipeline::new(retriever, reranker, generator);

        let output = pipeline.run_with_top_k("hello", 1).await.unwrap();

        assert_eq!(output.retrieved_docs.len(), 1);
        assert_eq!(output.reranked_docs.len(), 1);
        assert!(output.answer.contains("Q: hello"));
    }

    #[tokio::test]
    async fn pipeline_passes_top_k() {
        let retriever = Arc::new(FakeRetriever::new(vec![
            scored("1", "a", 0.9),
            scored("2", "b", 0.8),
            scored("3", "c", 0.7),
        ]));
        let top_k_ref = Arc::clone(&retriever.last_top_k);

        let pipeline = RagPipeline::new(retriever, Arc::new(IdentityReranker), Arc::new(FakeGenerator));
        let _ = pipeline.run_with_top_k("hello", 2).await.unwrap();

        let seen = *top_k_ref.lock().unwrap();
        assert_eq!(seen, Some(2));
    }

    #[tokio::test]
    async fn pipeline_reranker_changes_order() {
        let retriever = Arc::new(FakeRetriever::new(vec![
            scored("a", "first", 0.9),
            scored("b", "second", 0.8),
        ]));
        let pipeline = RagPipeline::new(retriever, Arc::new(ReverseReranker), Arc::new(FakeGenerator));

        let output = pipeline.run_with_top_k("hello", 2).await.unwrap();
        assert_eq!(output.retrieved_docs[0].document.id, "a");
        assert_eq!(output.reranked_docs[0].document.id, "b");
    }

    #[tokio::test]
    async fn pipeline_empty_retrieval_still_generates() {
        let retriever = Arc::new(FakeRetriever::new(vec![]));
        let pipeline = RagPipeline::new(retriever, Arc::new(IdentityReranker), Arc::new(FakeGenerator));

        let output = pipeline.run_with_top_k("hello", 1).await.unwrap();
        assert_eq!(output.retrieved_docs.len(), 0);
        assert!(output.answer.contains("ctx=0"));
    }

    #[tokio::test]
    async fn pipeline_propagates_errors() {
        let pipeline = RagPipeline::new(
            Arc::new(FailingRetriever),
            Arc::new(IdentityReranker),
            Arc::new(FakeGenerator),
        );

        let err = pipeline.run_with_top_k("hello", 1).await.unwrap_err();
        match err {
            AgentError::ExecutionFailed(msg) => assert!(msg.contains("retrieval failed")),
            other => panic!("unexpected error: {other:?}"),
        }
    }
}
