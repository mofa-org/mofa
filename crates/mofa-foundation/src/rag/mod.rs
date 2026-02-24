//! RAG (Retrieval-Augmented Generation) implementations
//!
//! Provides concrete implementations of the vector store trait defined
//! in mofa-kernel, along with utilities for document chunking.

pub mod adapter;
pub mod chunker;
pub mod similarity;
pub mod vector_store;

#[cfg(feature = "qdrant")]
pub mod qdrant_store;

pub use adapter::{GeneratorExt, PassthroughStreamingGenerator};
pub use chunker::{ChunkConfig, TextChunker};
pub use similarity::compute_similarity;
pub use vector_store::InMemoryVectorStore;

#[cfg(feature = "qdrant")]
pub use qdrant_store::{QdrantConfig, QdrantVectorStore};

// Re-export kernel types for convenience
pub use mofa_kernel::rag::{DocumentChunk, SearchResult, SimilarityMetric, VectorStore};
