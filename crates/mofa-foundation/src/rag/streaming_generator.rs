//! Streaming generator implementations
//!
//! Provides concrete implementations of streaming generators that wrap
//! existing generators to provide chunked output.

use async_trait::async_trait;
use futures::stream::{self, Stream};
use mofa_kernel::agent::error::AgentResult;
use mofa_kernel::rag::{GenerateInput, Generator, GeneratorChunk};
use std::pin::Pin;

/// A streaming generator that wraps an existing generator and yields the full
/// response as a single text chunk.
///
/// This is useful for adapting non-streaming generators to the streaming API.
pub struct PassthroughStreamingGenerator<G> {
    inner: G,
}

impl<G> PassthroughStreamingGenerator<G> {
    /// Create a new passthrough streaming generator
    pub fn new(generator: G) -> Self {
        Self { inner: generator }
    }
}

#[async_trait]
impl<G> Generator for PassthroughStreamingGenerator<G>
where
    G: Generator,
{
    async fn generate(&self, input: &GenerateInput) -> AgentResult<String> {
        self.inner.generate(input).await
    }

    async fn stream(
        &self,
        input: GenerateInput,
    ) -> AgentResult<Pin<Box<dyn Stream<Item = AgentResult<GeneratorChunk>> + Send>>> {
        let result = self.inner.generate(&input).await?;
        let stream = stream::once(async move { Ok(GeneratorChunk::Text(result)) });
        Ok(Box::pin(stream))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures::StreamExt;
    use mofa_kernel::rag::pipeline::Generator;

    struct MockGenerator;

    #[async_trait]
    impl Generator for MockGenerator {
        async fn generate(&self, _input: &GenerateInput) -> AgentResult<String> {
            Ok("mock response".to_string())
        }
    }

    #[tokio::test]
    async fn test_passthrough_generate() {
        let generator = PassthroughStreamingGenerator::new(MockGenerator);
        let input = GenerateInput {
            query: "test".to_string(),
            context: vec![],
            metadata: std::collections::HashMap::new(),
        };
        let result = generator.generate(&input).await.unwrap();
        assert_eq!(result, "mock response");
    }

    #[tokio::test]
    async fn test_passthrough_stream() {
        let generator = PassthroughStreamingGenerator::new(MockGenerator);
        let input = GenerateInput {
            query: "test".to_string(),
            context: vec![],
            metadata: std::collections::HashMap::new(),
        };
        let mut stream = generator.stream(input).await.unwrap();
        let chunks: Vec<_> = stream.collect().await;
        assert_eq!(chunks.len(), 1);
        match &chunks[0] {
            Ok(GeneratorChunk::Text(text)) => assert_eq!(text, "mock response"),
            _ => panic!("Expected text chunk"),
        }
    }
}
