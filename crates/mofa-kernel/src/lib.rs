// context module
pub mod context;

// plugin module
pub mod plugin;
pub use plugin::*;

// bus module
pub mod bus;
pub use bus::*;

// utils module
pub mod utils;

// logging module
pub mod logging;

// error module
pub mod error;
pub use error::{KernelError, KernelResult};

// core module
pub mod core;
pub use core::*;

// message module
pub mod message;

// MessageGraph module
pub mod message_graph;
pub use message_graph::*;

// Agent Framework (统一 Agent 框架)
pub mod agent;

// Global Configuration System (全局配置系统)
#[cfg(feature = "config")]
pub mod config;
#[cfg(feature = "config")]
pub use config::*;

// Storage traits (存储接口)
pub mod storage;
pub use storage::Storage;

// RAG traits (向量存储接口)
pub mod rag;
pub use rag::{
	Document,
	DocumentChunk,
	GenerateInput,
	Generator,
	RagPipeline,
	RagPipelineOutput,
	Reranker,
	Retriever,
	ScoredDocument,
	SearchResult,
	SimilarityMetric,
	VectorStore,
};


// Workflow traits (工作流接口)
pub mod workflow;
pub use workflow::*;

// Metrics traits for monitoring integration
pub mod metrics;
pub use metrics::*;
