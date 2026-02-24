//! RAG core data types
//!
//! Types used across the vector store and document chunking interfaces.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// A chunk of a document with its embedding vector and metadata.
///
/// This is the basic unit stored in a vector store. Documents are split
/// into chunks, each chunk is embedded into a vector, and stored along
/// with its text content and metadata for later retrieval.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentChunk {
    /// Unique identifier for this chunk
    pub id: String,
    /// The text content of this chunk
    pub text: String,
    /// The embedding vector for this chunk
    pub embedding: Vec<f32>,
    /// Arbitrary metadata (source file, page number, section title, etc.)
    pub metadata: HashMap<String, String>,
}

impl DocumentChunk {
    /// Create a new document chunk
    pub fn new(
        id: impl Into<String>,
        text: impl Into<String>,
        embedding: Vec<f32>,
    ) -> Self {
        Self {
            id: id.into(),
            text: text.into(),
            embedding,
            metadata: HashMap::new(),
        }
    }

    /// Add a metadata entry
    pub fn with_metadata(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.metadata.insert(key.into(), value.into());
        self
    }
}

/// Result returned from a vector similarity search.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    /// The id of the matched chunk
    pub id: String,
    /// The text content of the matched chunk
    pub text: String,
    /// Similarity score (higher is more similar, range depends on metric)
    pub score: f32,
    /// Metadata from the matched chunk
    pub metadata: HashMap<String, String>,
}

impl SearchResult {
    /// Create a new search result
    pub fn new(id: impl Into<String>, text: impl Into<String>, score: f32) -> Self {
        Self {
            id: id.into(),
            text: text.into(),
            score,
            metadata: HashMap::new(),
        }
    }

    /// Create from a document chunk with a score
    pub fn from_chunk(chunk: &DocumentChunk, score: f32) -> Self {
        Self {
            id: chunk.id.clone(),
            text: chunk.text.clone(),
            score,
            metadata: chunk.metadata.clone(),
        }
    }
}

/// Similarity metric used for comparing embedding vectors.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum SimilarityMetric {
    /// Cosine similarity (measures angle between vectors, range 0.0 to 1.0 for normalized vectors)
    #[default]
    Cosine,
    /// Euclidean distance (L2 distance, lower is more similar)
    Euclidean,
    /// Dot product (higher is more similar)
    DotProduct,
}

/// A chunk returned from a streaming generator.
///
/// This enum represents the individual chunks produced during streaming
/// generation, including text content and end-of-stream markers.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum GeneratorChunk {
    /// A text chunk containing a portion of the generated content.
    Text(String),
    /// Signal that generation is complete.
    End,
}

impl GeneratorChunk {
    /// Create a new text chunk
    pub fn text(content: impl Into<String>) -> Self {
        Self::Text(content.into())
    }

    /// Create an end chunk
    pub fn end() -> Self {
        Self::End
    }

    /// Check if this is an end chunk
    pub fn is_end(&self) -> bool {
        matches!(self, Self::End)
    }

    /// Extract text content if this is a text chunk
    pub fn as_text(&self) -> Option<&str> {
        match self {
            Self::Text(s) => Some(s),
            Self::End => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_document_chunk_creation() {
        let chunk = DocumentChunk::new("chunk-1", "hello world", vec![0.1, 0.2, 0.3])
            .with_metadata("source", "test.txt")
            .with_metadata("page", "1");

        assert_eq!(chunk.id, "chunk-1");
        assert_eq!(chunk.text, "hello world");
        assert_eq!(chunk.embedding.len(), 3);
        assert_eq!(chunk.metadata.get("source").unwrap(), "test.txt");
        assert_eq!(chunk.metadata.get("page").unwrap(), "1");
    }

    #[test]
    fn test_search_result_from_chunk() {
        let chunk = DocumentChunk::new("chunk-1", "hello world", vec![0.1, 0.2, 0.3])
            .with_metadata("source", "test.txt");

        let result = SearchResult::from_chunk(&chunk, 0.95);
        assert_eq!(result.id, "chunk-1");
        assert_eq!(result.text, "hello world");
        assert_eq!(result.score, 0.95);
        assert_eq!(result.metadata.get("source").unwrap(), "test.txt");
    }

    #[test]
    fn test_similarity_metric_default() {
        assert_eq!(SimilarityMetric::default(), SimilarityMetric::Cosine);
    }

    #[test]
    fn test_generator_chunk_text() {
        let chunk = GeneratorChunk::text("hello");
        assert!(!chunk.is_end());
        assert_eq!(chunk.as_text(), Some("hello"));
    }

    #[test]
    fn test_generator_chunk_end() {
        let chunk = GeneratorChunk::end();
        assert!(chunk.is_end());
        assert_eq!(chunk.as_text(), None);
    }

    #[test]
    fn test_generator_chunk_owned() {
        let chunk = GeneratorChunk::text("world");
        let text = chunk.as_text().map(|s| s.to_uppercase());
        assert_eq!(text, Some("WORLD".to_string()));
    }
}
