//! RAG (Retrieval-Augmented Generation) traits and types
//!
//! Defines the core abstractions for vector storage and document chunking
//! used in RAG pipelines. Concrete implementations live in mofa-foundation.

pub mod pipeline;
pub mod types;
pub mod vector_store;
pub mod pipeline;

pub use pipeline::{
    Generator, GeneratorChunk, RagPipeline, RagPipelineOutput, RerankInput, RerankOutput, Reranker,
    RetrieveInput, RetrieveOutput, Retriever, ScoredDocument,
};
pub use types::{DocumentChunk, SearchResult, SimilarityMetric, Document, ScoredDocument, GenerateInput};
pub use vector_store::VectorStore;
