//! Hybrid retrieval module
//!
//! Provides hybrid dense + sparse retrieval using Reciprocal Rank Fusion (RRF).
//!
//! # Architecture
//!
//! ```text
//! query
//!  ↓
//! dense retriever
//!  ↓
//! BM25 retriever
//!  ↓
//! reciprocal rank fusion
//!  ↓
//! top-k results
//! ```
//!
//! # Usage
//!
//! ## Basic usage with HybridSearchPipeline
//!
//! ```ignore
//! use mofa_foundation::rag::hybrid::{HybridSearchPipeline, HybridRetrieverConfig};
//! use mofa_foundation::rag::{InMemoryVectorStore, Bm25Retriever};
//! use std::sync::Arc;
//!
//! let dense_store = Arc::new(InMemoryVectorStore::new(384));
//! let sparse_retriever = Arc::new(Bm25Retriever::new());
//! let embedder = Arc::new(my_embedder);
//!
//! let hybrid = HybridSearchPipeline::new(dense_store, sparse_retriever, embedder);
//! let results = hybrid.retrieve("query", 5).await?;
//! ```
//!
//! ## Generic hybrid retriever
//!
//! ```ignore
//! use mofa_foundation::rag::hybrid::GenericHybridRetriever;
//!
//! let retriever_a = Arc::new(my_dense_retriever); // implements Retriever
//! let retriever_b = Arc::new(my_sparse_retriever); // implements Retriever
//!
//! let hybrid = GenericHybridRetriever::new(retriever_a, retriever_b);
//! let results = hybrid.retrieve("query", 5).await?;
//! ```

pub mod hybrid_retriever;
pub mod rrf;

pub use hybrid_retriever::{
    GenericHybridRetriever, HybridRetrieverConfig, HybridSearchPipeline,
};
pub use rrf::{reciprocal_rank_fusion, reciprocal_rank_fusion_default, DEFAULT_RRF_K};

// Re-export the kernel trait
pub use mofa_kernel::rag::HybridRetriever;
