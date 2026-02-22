//! RAG (Retrieval-Augmented Generation) implementations
//!
//! Provides concrete implementations of the vector store trait defined
//! in mofa-kernel, along with utilities for document chunking.

pub mod chunker;
pub mod similarity;
pub mod vector_store;

pub use chunker::{ChunkConfig, TextChunker};
pub use similarity::compute_similarity;
pub use vector_store::InMemoryVectorStore;

// Re-export kernel types for convenience
pub use mofa_kernel::rag::{DocumentChunk, SearchResult, SimilarityMetric, VectorStore};
