//! RAG pipeline orchestration traits
//!
//! Defines the core traits for retrieval, reranking, and generation
//! in RAG pipelines. Concrete implementations live in mofa-foundation.

use crate::agent::error::AgentResult;
use crate::rag::types::{DocumentChunk, SearchResult};
use async_trait::async_trait;
use futures::Stream;
use serde::{Deserialize, Serialize};
use std::pin::Pin;

/// Input to a retriever
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetrieveInput {
    /// The query text to retrieve documents for
    pub query: String,
    /// Maximum number of documents to retrieve
    pub top_k: usize,
}

/// Output from a retriever
pub type RetrieveOutput = Vec<ScoredDocument>;

/// A document with a relevance score
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScoredDocument {
    /// The document chunk
    pub document: DocumentChunk,
    /// Relevance score (higher is better)
    pub score: f32,
    /// Optional source label (e.g., "dense", "sparse", "hybrid")
    pub source: Option<String>,
}

/// Input to a reranker
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RerankInput {
    /// The query text
    pub query: String,
    /// Documents to rerank
    pub documents: Vec<ScoredDocument>,
    /// Maximum number of documents to return after reranking
    pub top_k: usize,
}

/// Output from a reranker
pub type RerankOutput = Vec<ScoredDocument>;

/// Input to a generator
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenerateInput {
    /// The query text
    pub query: String,
    /// Retrieved and optionally reranked documents
    pub documents: Vec<ScoredDocument>,
}

/// A chunk of generated text from a streaming generator
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum GeneratorChunk {
    /// A piece of generated text
    Text(String),
    /// End of generation
    End,
}

/// Output from a RAG pipeline run
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RagPipelineOutput {
    /// The generated response
    pub response: String,
    /// The documents used for generation
    pub documents: Vec<ScoredDocument>,
}

/// Retriever trait for finding relevant documents
#[async_trait]
pub trait Retriever: Send + Sync {
    async fn retrieve(&self, input: RetrieveInput) -> AgentResult<RetrieveOutput>;
}

/// Reranker trait for improving document ranking
#[async_trait]
pub trait Reranker: Send + Sync {
    async fn rerank(&self, input: RerankInput) -> AgentResult<RerankOutput>;
}

/// Generator trait for producing text from documents
#[async_trait]
pub trait Generator: Send + Sync {
    async fn generate(&self, input: GenerateInput) -> AgentResult<String>;

    /// Stream generated text in chunks
    async fn stream(
        &self,
        input: GenerateInput,
    ) -> AgentResult<Pin<Box<dyn Stream<Item = AgentResult<GeneratorChunk>> + Send>>> {
        // Default implementation: generate all at once and yield as single chunk
        let result = self.generate(input).await?;
        let stream = futures::stream::once(async move { Ok(GeneratorChunk::Text(result)) });
        Ok(Box::pin(stream))
    }
}

/// Orchestrates retrieval, optional reranking, and generation
pub struct RagPipeline<R, Re, G> {
    retriever: R,
    reranker: Option<Re>,
    generator: G,
}

impl<R, Re, G> RagPipeline<R, Re, G>
where
    R: Retriever,
    Re: Reranker,
    G: Generator,
{
    /// Create a new RAG pipeline
    pub fn new(retriever: R, reranker: Option<Re>, generator: G) -> Self {
        Self {
            retriever,
            reranker,
            generator,
        }
    }

    /// Run the full pipeline with default top_k
    pub async fn run(&self, query: impl Into<String>) -> AgentResult<RagPipelineOutput> {
        self.run_with_top_k(query, 5).await
    }

    /// Run the full pipeline with specified top_k
    pub async fn run_with_top_k(
        &self,
        query: impl Into<String>,
        top_k: usize,
    ) -> AgentResult<RagPipelineOutput> {
        let query = query.into();

        // Retrieve
        let retrieved = self
            .retriever
            .retrieve(RetrieveInput {
                query: query.clone(),
                top_k,
            })
            .await?;

        // Rerank if available
        let documents = if let Some(reranker) = &self.reranker {
            reranker
                .rerank(RerankInput {
                    query: query.clone(),
                    documents: retrieved,
                    top_k,
                })
                .await?
        } else {
            retrieved
        };

        // Generate
        let response = self
            .generator
            .generate(GenerateInput {
                query,
                documents: documents.clone(),
            })
            .await?;

        Ok(RagPipelineOutput {
            response,
            documents,
        })
    }

    /// Run the pipeline with streaming generation
    pub async fn run_streaming(
        &self,
        query: impl Into<String>,
        top_k: usize,
    ) -> AgentResult<(
        Vec<ScoredDocument>,
        Pin<Box<dyn Stream<Item = AgentResult<GeneratorChunk>> + Send>>,
    )> {
        let query = query.into();

        // Retrieve
        let retrieved = self
            .retriever
            .retrieve(RetrieveInput {
                query: query.clone(),
                top_k,
            })
            .await?;

        // Rerank if available
        let documents = if let Some(reranker) = &self.reranker {
            reranker
                .rerank(RerankInput {
                    query: query.clone(),
                    documents: retrieved,
                    top_k,
                })
                .await?
        } else {
            retrieved
        };

        // Stream generate
        let stream = self
            .generator
            .stream(GenerateInput {
                query,
                documents: documents.clone(),
            })
            .await?;

        Ok((documents, stream))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures::StreamExt;

    struct MockRetriever;
    struct MockReranker;
    struct MockGenerator;

    #[async_trait]
    impl Retriever for MockRetriever {
        async fn retrieve(&self, _input: RetrieveInput) -> AgentResult<RetrieveOutput> {
            Ok(vec![ScoredDocument {
                document: DocumentChunk::new("test", "test content", vec![1.0]),
                score: 1.0,
                source: None,
            }])
        }
    }

    #[async_trait]
    impl Reranker for MockReranker {
        async fn rerank(&self, input: RerankInput) -> AgentResult<RerankOutput> {
            Ok(input.documents)
        }
    }

    #[async_trait]
    impl Generator for MockGenerator {
        async fn generate(&self, _input: GenerateInput) -> AgentResult<String> {
            Ok("generated response".to_string())
        }
    }

    #[tokio::test]
    async fn test_pipeline_run() {
        let pipeline = RagPipeline::new(MockRetriever, Some(MockReranker), MockGenerator);
        let result = pipeline.run("test query").await.unwrap();
        assert_eq!(result.response, "generated response");
        assert_eq!(result.documents.len(), 1);
    }

    #[tokio::test]
    async fn test_pipeline_run_streaming() {
        let pipeline = RagPipeline::new(MockRetriever, Some(MockReranker), MockGenerator);
        let (documents, mut stream) = pipeline.run_streaming("test query", 5).await.unwrap();
        assert_eq!(documents.len(), 1);

        let chunks: Vec<_> = stream.collect().await;
        assert_eq!(chunks.len(), 1);
        match &chunks[0] {
            Ok(GeneratorChunk::Text(text)) => assert_eq!(text, "generated response"),
            _ => panic!("Expected text chunk"),
        }
    }
}
