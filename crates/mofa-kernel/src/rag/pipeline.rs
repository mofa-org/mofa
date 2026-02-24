//! RAG Pipeline traits
//!
//! Defines the Generator trait for LLM-powered RAG pipelines with streaming support.

use crate::rag::GeneratorChunk;
use async_trait::async_trait;
use futures::Stream;
use std::pin::Pin;

/// Error type for RAG pipeline operations
#[derive(Debug, thiserror::Error)]
pub enum PipelineError {
    #[error("Generation error: {0}")]
    Generation(String),
    #[error("Retrieval error: {0}")]
    Retrieval(String),
    #[error("Streaming error: {0}")]
    Stream(String),
}

/// Result type for pipeline operations
pub type PipelineResult<T> = Result<T, PipelineError>;

/// Stream type for generator output
pub type GeneratorStream = Pin<Box<dyn Stream<Item = PipelineResult<GeneratorChunk>> + Send>>;

/// Generator trait for RAG pipelines
///
/// This trait defines the interface for generating responses using LLMs
/// in a Retrieval-Augmented Generation pipeline. It supports both blocking
/// generation and streaming generation.
#[async_trait]
pub trait Generator: Send + Sync {
    /// Generate a response from the given context and query.
    ///
    /// This is a blocking call that returns the complete generated response.
    ///
    /// # Arguments
    /// * `context` - The retrieved context from the vector store
    /// * `query` - The user's query
    ///
    /// # Returns
    /// The generated text response
    async fn generate(&self, context: &str, query: &str) -> PipelineResult<String>;

    /// Stream a response from the given context and query.
    ///
    /// This returns a stream of GeneratorChunk items that can be processed
    /// as they become available, enabling real-time token-by-token processing.
    ///
    /// # Arguments
    /// * `context` - The retrieved context from the vector store
    /// * `query` - The user's query
    ///
    /// # Returns
    /// A stream of GeneratorChunk items
    async fn stream(
        &self,
        context: &str,
        query: &str,
    ) -> PipelineResult<GeneratorStream>;

    /// Check if this generator supports streaming.
    ///
    /// Returns true if the stream method is implemented and can produce
    /// incremental output. If false, callers should fall back to generate.
    fn supports_streaming(&self) -> bool {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures::StreamExt;

    /// A simple mock generator for testing
    struct MockGenerator {
        response: String,
        stream_chunks: Vec<GeneratorChunk>,
    }

    #[async_trait]
    impl Generator for MockGenerator {
        async fn generate(&self, _context: &str, _query: &str) -> PipelineResult<String> {
            Ok(self.response.clone())
        }

        async fn stream(
            &self,
            _context: &str,
            _query: &str,
        ) -> PipelineResult<GeneratorStream> {
            let chunks: Vec<GeneratorChunk> = self.stream_chunks.clone();
            let stream = futures::stream::iter(
                chunks
                    .into_iter()
                    .map(|chunk| Ok::<_, PipelineError>(chunk)),
            );
            Ok(Box::pin(stream))
        }

        fn supports_streaming(&self) -> bool {
            !self.stream_chunks.is_empty()
        }
    }

    #[tokio::test]
    async fn test_mock_generator_generate() {
        let generator = MockGenerator {
            response: "Hello, world!".to_string(),
            stream_chunks: vec![],
        };

        let result = generator.generate("context", "query").await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "Hello, world!");
    }

    #[tokio::test]
    async fn test_mock_generator_stream() {
        let generator = MockGenerator {
            response: "Hello, world!".to_string(),
            stream_chunks: vec![
                GeneratorChunk::text("Hello"),
                GeneratorChunk::text(", "),
                GeneratorChunk::text("world"),
                GeneratorChunk::text("!"),
                GeneratorChunk::end(),
            ],
        };

        let stream = generator.stream("context", "query").await.unwrap();

        let collected: Vec<_> = stream.collect().await;
        assert_eq!(collected.len(), 5);
    }

    #[tokio::test]
    async fn test_generator_supports_streaming() {
        let generator_with_stream = MockGenerator {
            response: "test".to_string(),
            stream_chunks: vec![GeneratorChunk::text("test")],
        };

        let generator_without_stream = MockGenerator {
            response: "test".to_string(),
            stream_chunks: vec![],
        };

        assert!(generator_with_stream.supports_streaming());
        assert!(!generator_without_stream.supports_streaming());
    }
}
