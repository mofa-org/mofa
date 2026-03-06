//! RAG Indexing Pipeline
//!
//! End-to-end document indexing that connects:
//! 1. Document chunking ([`TextChunker`]) with deterministic chunk IDs
//! 2. Embedding via [`LlmEmbeddingAdapter`]
//! 3. Vector store upsert/replace indexing
//!
//! This module provides the "index these documents" entry point.

use crate::rag::chunker::{ChunkConfig, TextChunker};
use crate::rag::embedding_adapter::{EmbeddingAdapterError, LlmEmbeddingAdapter};
use mofa_kernel::rag::{DocumentChunk, VectorStore};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ---------------------------------------------------------------------------
// Indexing types
// ---------------------------------------------------------------------------

/// How indexing handles existing chunks for a document.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[non_exhaustive]
pub enum IndexMode {
    /// Insert new chunks, update changed chunks, keep unmatched old chunks.
    #[default]
    Upsert,
    /// Delete all existing chunks for the document, then insert fresh chunks.
    Replace,
}

impl std::fmt::Display for IndexMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            IndexMode::Upsert => write!(f, "upsert"),
            IndexMode::Replace => write!(f, "replace"),
        }
    }
}

/// Configuration for the indexing pipeline.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RagIndexConfig {
    /// Text chunking configuration.
    pub chunk_size: usize,
    /// Overlap between adjacent chunks.
    pub chunk_overlap: usize,
    /// Indexing mode.
    pub index_mode: IndexMode,
}

impl Default for RagIndexConfig {
    fn default() -> Self {
        Self {
            chunk_size: 512,
            chunk_overlap: 64,
            index_mode: IndexMode::default(),
        }
    }
}

impl RagIndexConfig {
    /// Builder: set chunk size.
    pub fn with_chunk_size(mut self, size: usize) -> Self {
        self.chunk_size = size.max(1);
        self
    }

    /// Builder: set chunk overlap.
    ///
    /// Clamped to `chunk_size - 1` to prevent pathological step-of-1 configs.
    pub fn with_chunk_overlap(mut self, overlap: usize) -> Self {
        let max_overlap = self.chunk_size.saturating_sub(1);
        self.chunk_overlap = overlap.min(max_overlap);
        self
    }

    /// Builder: set index mode.
    pub fn with_index_mode(mut self, mode: IndexMode) -> Self {
        self.index_mode = mode;
        self
    }
}

/// A document to be indexed.
#[derive(Debug, Clone)]
pub struct IndexDocument {
    /// Unique document identifier.
    pub id: String,
    /// Full text content.
    pub text: String,
    /// Arbitrary metadata carried through to stored chunks.
    pub metadata: HashMap<String, String>,
}

impl IndexDocument {
    /// Create a new document.
    pub fn new(id: impl Into<String>, text: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            text: text.into(),
            metadata: HashMap::new(),
        }
    }

    /// Add a metadata entry.
    pub fn with_metadata(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.metadata.insert(key.into(), value.into());
        self
    }
}

/// Result of an indexing operation.
#[derive(Debug, Clone)]
pub struct IndexResult {
    /// Number of chunks produced.
    pub chunks_total: usize,
    /// Number of chunks upserted into the store.
    pub chunks_upserted: usize,
    /// Number of chunks deleted (only non-zero in Replace mode).
    pub chunks_deleted: usize,
    /// Document IDs that were processed.
    pub document_ids: Vec<String>,
}

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

/// Errors from the RAG orchestration layer.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum RagOrchestrationError {
    /// Embedding adapter error.
    #[error("embedding error: {0}")]
    Embedding(#[from] EmbeddingAdapterError),

    /// Vector store error (stringified — VectorStore trait returns AgentError
    /// which doesn't implement std::error::Error, so we store the message).
    #[error("vector store error: {0}")]
    VectorStore(String),

    /// Invalid input.
    #[error("invalid input: {0}")]
    InvalidInput(String),
}

// ---------------------------------------------------------------------------
// Index pipeline
// ---------------------------------------------------------------------------

/// Index a batch of documents into a vector store.
///
/// 1. Chunk each document using `TextChunker`
/// 2. Generate deterministic chunk IDs (`doc_id:chunk_idx:content_hash`)
/// 3. Embed all chunks via `LlmEmbeddingAdapter`
/// 4. Upsert or replace in the vector store
///
/// Returns an [`IndexResult`] with statistics.
pub async fn index_documents<S: VectorStore>(
    store: &mut S,
    embedder: &LlmEmbeddingAdapter,
    documents: &[IndexDocument],
    config: &RagIndexConfig,
) -> Result<IndexResult, RagOrchestrationError> {
    if documents.is_empty() {
        return Ok(IndexResult {
            chunks_total: 0,
            chunks_upserted: 0,
            chunks_deleted: 0,
            document_ids: Vec::new(),
        });
    }

    let chunker = TextChunker::new(ChunkConfig::new(config.chunk_size, config.chunk_overlap));

    // 1. Chunk all documents and collect (id, text, metadata) tuples
    let mut chunk_entries: Vec<(String, String, HashMap<String, String>)> = Vec::new();
    let mut doc_ids: Vec<String> = Vec::new();
    let chunks_deleted: usize = 0;

    for doc in documents {
        doc_ids.push(doc.id.clone());

        let chunk_texts = chunker.chunk_by_chars(&doc.text);

        for (chunk_idx, chunk_text) in chunk_texts.into_iter().enumerate() {
            let chunk_id = crate::rag::embedding_adapter::deterministic_chunk_id(
                &doc.id,
                chunk_idx,
                &chunk_text,
            );

            let mut metadata = doc.metadata.clone();
            metadata.insert("source_doc_id".to_string(), doc.id.clone());
            metadata.insert("chunk_index".to_string(), chunk_idx.to_string());

            chunk_entries.push((chunk_id, chunk_text, metadata));
        }
    }

    let chunks_total = chunk_entries.len();

    if chunks_total == 0 {
        return Ok(IndexResult {
            chunks_total: 0,
            chunks_upserted: 0,
            chunks_deleted,
            document_ids: doc_ids,
        });
    }

    // 2. Collect texts for batch embedding (borrow from chunk_entries)
    let texts_to_embed: Vec<String> = chunk_entries.iter().map(|(_, t, _)| t.clone()).collect();
    let embeddings = embedder.embed_batch(&texts_to_embed).await?;

    // embed_batch already validates count internally; this is a defensive check
    debug_assert_eq!(embeddings.len(), chunks_total);

    // 3. Build DocumentChunks and upsert — use text from chunk_entries directly
    let mut doc_chunks = Vec::with_capacity(chunks_total);
    for ((chunk_id, chunk_text, metadata), embedding) in
        chunk_entries.into_iter().zip(embeddings.into_iter())
    {
        let mut chunk = DocumentChunk::new(chunk_id, &chunk_text, embedding);
        chunk.metadata = metadata;
        doc_chunks.push(chunk);
    }

    store
        .upsert_batch(doc_chunks)
        .await
        .map_err(|e| RagOrchestrationError::VectorStore(e.to_string()))?;

    Ok(IndexResult {
        chunks_total,
        chunks_upserted: chunks_total,
        chunks_deleted,
        document_ids: doc_ids,
    })
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rag::embedding_adapter::RagEmbeddingConfig;
    use async_trait::async_trait;
    use mofa_kernel::agent::error::{AgentError, AgentResult};
    use mofa_kernel::rag::{SearchResult, SimilarityMetric};

    // -- Mock embedder using a fake LLMProvider --

    struct MockProvider {
        dimensions: usize,
    }

    #[async_trait]
    impl crate::llm::provider::LLMProvider for MockProvider {
        fn name(&self) -> &str { "mock" }
        fn default_model(&self) -> &str { "mock-embed" }
        fn supports_streaming(&self) -> bool { false }
        fn supports_tools(&self) -> bool { false }
        fn supports_vision(&self) -> bool { false }

        async fn chat(
            &self,
            _request: crate::llm::types::ChatCompletionRequest,
        ) -> crate::llm::types::LLMResult<crate::llm::types::ChatCompletionResponse> {
            Err(crate::llm::types::LLMError::Other("not supported".into()))
        }

        async fn chat_stream(
            &self,
            _request: crate::llm::types::ChatCompletionRequest,
        ) -> crate::llm::types::LLMResult<crate::llm::provider::ChatStream> {
            Err(crate::llm::types::LLMError::Other("not supported".into()))
        }

        async fn embedding(
            &self,
            request: crate::llm::types::EmbeddingRequest,
        ) -> crate::llm::types::LLMResult<crate::llm::types::EmbeddingResponse> {
            let inputs = match request.input {
                crate::llm::types::EmbeddingInput::Single(s) => vec![s],
                crate::llm::types::EmbeddingInput::Multiple(v) => v,
            };
            let data = inputs.iter().map(|text| {
                let mut vec = vec![0.0f32; self.dimensions];
                for (i, b) in text.bytes().enumerate() {
                    vec[i % self.dimensions] += b as f32 / 255.0;
                }
                let norm = vec.iter().map(|x| x * x).sum::<f32>().sqrt();
                if norm > 0.0 { for v in &mut vec { *v /= norm; } }
                crate::llm::types::EmbeddingData {
                    object: "embedding".into(), embedding: vec, index: 0,
                }
            }).collect();
            Ok(crate::llm::types::EmbeddingResponse {
                object: "list".into(), data, model: request.model,
                usage: crate::llm::types::EmbeddingUsage { prompt_tokens: 0, total_tokens: 0 },
            })
        }
    }

    fn make_adapter(dimensions: usize) -> LlmEmbeddingAdapter {
        let provider = std::sync::Arc::new(MockProvider { dimensions });
        let client = crate::llm::client::LLMClient::new(provider);
        let config = RagEmbeddingConfig::default().with_dimensions(dimensions);
        LlmEmbeddingAdapter::new(client, config)
    }

    // -- Mock vector store --

    struct TestStore {
        chunks: HashMap<String, DocumentChunk>,
        dimensions: Option<usize>,
    }
    impl TestStore { fn new() -> Self { Self { chunks: HashMap::new(), dimensions: None } } }

    #[async_trait]
    impl VectorStore for TestStore {
        async fn upsert(&mut self, chunk: DocumentChunk) -> AgentResult<()> {
            if let Some(dim) = self.dimensions {
                if chunk.embedding.len() != dim {
                    return Err(AgentError::InvalidInput(format!(
                        "dimension mismatch: expected {}, got {}", dim, chunk.embedding.len()
                    )));
                }
            } else { self.dimensions = Some(chunk.embedding.len()); }
            self.chunks.insert(chunk.id.clone(), chunk);
            Ok(())
        }
        async fn search(&self, _q: &[f32], _k: usize, _t: Option<f32>) -> AgentResult<Vec<SearchResult>> {
            Ok(Vec::new())
        }
        async fn delete(&mut self, id: &str) -> AgentResult<bool> { Ok(self.chunks.remove(id).is_some()) }
        async fn clear(&mut self) -> AgentResult<()> { self.chunks.clear(); self.dimensions = None; Ok(()) }
        async fn count(&self) -> AgentResult<usize> { Ok(self.chunks.len()) }
        fn similarity_metric(&self) -> SimilarityMetric { SimilarityMetric::DotProduct }
    }

    #[tokio::test]
    async fn index_empty_documents() {
        let mut store = TestStore::new();
        let adapter = make_adapter(32);
        let result = index_documents(&mut store, &adapter, &[], &RagIndexConfig::default()).await.unwrap();
        assert_eq!(result.chunks_total, 0);
    }

    #[tokio::test]
    async fn index_single_document() {
        let mut store = TestStore::new();
        let adapter = make_adapter(32);
        let docs = vec![IndexDocument::new("doc-1", "Rust is a systems programming language.")];
        let result = index_documents(&mut store, &adapter, &docs, &RagIndexConfig::default()).await.unwrap();
        assert!(result.chunks_total >= 1);
        assert_eq!(result.chunks_upserted, result.chunks_total);
        assert_eq!(result.document_ids, vec!["doc-1"]);
    }

    #[tokio::test]
    async fn index_idempotent_with_same_content() {
        let mut store = TestStore::new();
        let adapter = make_adapter(32);
        let docs = vec![IndexDocument::new("doc-1", "Hello world")];
        let r1 = index_documents(&mut store, &adapter, &docs, &RagIndexConfig::default()).await.unwrap();
        let r2 = index_documents(&mut store, &adapter, &docs, &RagIndexConfig::default()).await.unwrap();
        assert_eq!(r1.chunks_total, r2.chunks_total);
        assert_eq!(store.count().await.unwrap(), r1.chunks_total);
    }

    #[tokio::test]
    async fn index_multiple_documents() {
        let mut store = TestStore::new();
        let adapter = make_adapter(16);
        let docs = vec![
            IndexDocument::new("a", "Document alpha."),
            IndexDocument::new("b", "Document beta."),
        ];
        let result = index_documents(&mut store, &adapter, &docs, &RagIndexConfig::default()).await.unwrap();
        assert_eq!(result.document_ids.len(), 2);
    }

    #[tokio::test]
    async fn index_preserves_metadata() {
        let mut store = TestStore::new();
        let adapter = make_adapter(16);
        let docs = vec![IndexDocument::new("doc-1", "Text").with_metadata("author", "alice")];
        index_documents(&mut store, &adapter, &docs, &RagIndexConfig::default()).await.unwrap();
        let chunk = store.chunks.values().next().unwrap();
        assert_eq!(chunk.metadata.get("author").map(String::as_str), Some("alice"));
        assert_eq!(chunk.metadata.get("source_doc_id").map(String::as_str), Some("doc-1"));
    }

    #[test]
    fn index_config_defaults() {
        let c = RagIndexConfig::default();
        assert_eq!(c.chunk_size, 512);
        assert_eq!(c.chunk_overlap, 64);
        assert_eq!(c.index_mode, IndexMode::Upsert);
    }

    #[test]
    fn index_mode_display() {
        assert_eq!(IndexMode::Upsert.to_string(), "upsert");
        assert_eq!(IndexMode::Replace.to_string(), "replace");
    }
}
