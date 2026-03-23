//! RAG (Retrieval-Augmented Generation) traits and types
//!
//! Defines the core abstractions for vector storage and document chunking
//! used in RAG pipelines. Concrete implementations live in mofa-foundation.

pub mod hybrid;
pub mod pipeline;
pub mod types;
pub mod vector_store;

pub use hybrid::HybridRetriever;
pub use pipeline::{
    Generator, GeneratorChunk, RagPipeline, RagPipelineOutput, Reranker, Retriever,
};
pub use types::{
    Document, DocumentChunk, GenerateInput, ScoredDocument, SearchResult, SimilarityMetric,
};
pub use vector_store::VectorStore;
