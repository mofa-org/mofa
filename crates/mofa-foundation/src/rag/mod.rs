//! RAG (Retrieval-Augmented Generation) implementations
//!
//! Provides concrete implementations of the vector store trait defined
//! in mofa-kernel, along with utilities for document chunking.

pub mod chunker;
pub mod pipeline_adapters;
pub mod default_reranker;
pub mod similarity;
pub mod streaming_generator;
pub mod vector_store;

#[cfg(feature = "qdrant")]
pub mod qdrant_store;

pub use chunker::{ChunkConfig, TextChunker};
pub use pipeline_adapters::{InMemoryRetriever, SimpleGenerator};
pub use default_reranker::IdentityReranker;
pub use similarity::compute_similarity;
pub use streaming_generator::PassthroughStreamingGenerator;
pub use vector_store::InMemoryVectorStore;

#[cfg(feature = "qdrant")]
pub use qdrant_store::{QdrantConfig, QdrantVectorStore};

// Re-export kernel types for convenience
pub use mofa_kernel::rag::{
    Document, DocumentChunk, GenerateInput, Generator, GeneratorChunk, RagPipeline,
    RagPipelineOutput, Reranker, Retriever, ScoredDocument, SearchResult, SimilarityMetric,
    VectorStore,
};
