//! RAG (Retrieval-Augmented Generation) traits and types
//!
//! Defines the core abstractions for vector storage and document chunking
//! used in RAG pipelines. Concrete implementations live in mofa-foundation.

pub mod types;
pub mod vector_store;
pub mod pipeline;

pub use types::{DocumentChunk, SearchResult, SimilarityMetric, Document, ScoredDocument, GenerateInput};
pub use vector_store::VectorStore;
pub use pipeline::{Generator, RagPipeline, RagPipelineOutput, Reranker, Retriever};
