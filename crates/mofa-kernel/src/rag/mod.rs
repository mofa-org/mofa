//! RAG (Retrieval-Augmented Generation) traits and types
//!
//! Defines the core abstractions for vector storage and document chunking
//! used in RAG pipelines. Concrete implementations live in mofa-foundation.

pub mod pipeline;
pub mod types;
pub mod vector_store;

pub use types::{DocumentChunk, GeneratorChunk, SearchResult, SimilarityMetric};
pub use vector_store::VectorStore;
pub use pipeline::{Generator, GeneratorStream, PipelineError, PipelineResult};
