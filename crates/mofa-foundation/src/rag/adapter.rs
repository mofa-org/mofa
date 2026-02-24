//! RAG Pipeline Adapters
//!
//! Provides adapters and utilities for RAG pipelines including
//! streaming generators and pipeline helpers.

use mofa_kernel::rag::{Generator, GeneratorChunk, GeneratorStream, PipelineError, PipelineResult};
use async_trait::async_trait;
use std::pin::Pin;
use std::sync::Arc;

/// A passthrough streaming generator that wraps a non-streaming generator.
///
/// This adapter provides streaming support by calling the blocking `generate`
/// method and yielding the result as a single chunk followed by End.
/// This is useful for generators that don't natively support streaming.
#[derive(Debug, Clone)]
pub struct PassthroughStreamingGenerator<G: Generator> {
    inner: Arc<G>,
}

impl<G: Generator> PassthroughStreamingGenerator<G> {
    /// Create a new passthrough streaming generator
    pub fn new(generator: G) -> Self {
        Self {
            inner: Arc::new(generator),
        }
    }

    /// Get a reference to the inner generator
    pub fn inner(&self) -> &G {
        &self.inner
    }
}

#[async_trait]
impl<G: Generator> Generator for PassthroughStreamingGenerator<G> {
    async fn generate(&self, context: &str, query: &str) -> PipelineResult<String> {
        self.inner.generate(context, query).await
    }

    async fn stream(
        &self,
        context: &str,
        query: &str,
    ) -> PipelineResult<GeneratorStream> {
        // Get the complete response and yield it as a single chunk
        let response = self.inner.generate(context, query).await?;
        
        let chunks = vec![
            GeneratorChunk::text(response),
            GeneratorChunk::end(),
        ];
        
        let stream = futures::stream::iter(
            chunks.into_iter().map(Ok::<_, PipelineError>)
        );
        
        Ok(Box::pin(stream))
    }

    fn supports_streaming(&self) -> bool {
        // This adapter always provides streaming (by falling back to generate)
        true
    }
}

/// Extension trait for adding streaming support to generators
pub trait GeneratorExt: Generator {
    /// Convert this generator to one that supports streaming
    ///
    /// If the generator already supports streaming, returns the same generator.
    /// Otherwise, wraps it in a PassthroughStreamingGenerator.
    fn with_streaming(self) -> PassthroughStreamingGenerator<Self>
    where
        Self: Sized,
    {
        PassthroughStreamingGenerator::new(self)
    }
}

impl<G: Generator> GeneratorExt for G {}

#[cfg(test)]
mod tests {
    use super::*;
    use futures::StreamExt;
    use mofa_kernel::rag::PipelineError;

    /// A simple mock generator for testing
    struct MockGenerator {
        response: String,
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
            Err(PipelineError::Stream("Not implemented".to_string()))
        }

        fn supports_streaming(&self) -> bool {
            false
        }
    }

    #[tokio::test]
    async fn test_passthrough_generator_generate() {
        let generator = MockGenerator {
            response: "Test response".to_string(),
        };
        let passthrough = PassthroughStreamingGenerator::new(generator);

        let result = passthrough.generate("context", "query").await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "Test response");
    }

    #[tokio::test]
    async fn test_passthrough_generator_stream() {
        let generator = MockGenerator {
            response: "Test response".to_string(),
        };
        let passthrough = PassthroughStreamingGenerator::new(generator);

        let stream = passthrough.stream("context", "query").await.unwrap();
        let collected: Vec<_> = stream.collect().await;
        
        assert_eq!(collected.len(), 2);
        assert_eq!(collected[0].as_ref().unwrap().as_text(), Some("Test response"));
        assert!(collected[1].as_ref().unwrap().is_end());
    }

    #[tokio::test]
    async fn test_passthrough_supports_streaming() {
        let generator = MockGenerator {
            response: "Test".to_string(),
        };
        let passthrough = PassthroughStreamingGenerator::new(generator);

        assert!(passthrough.supports_streaming());
    }

    #[tokio::test]
    async fn test_generator_ext() {
        let generator = MockGenerator {
            response: "Test".to_string(),
        };
        
        let streaming = generator.with_streaming();
        assert!(streaming.supports_streaming());
    }
}
