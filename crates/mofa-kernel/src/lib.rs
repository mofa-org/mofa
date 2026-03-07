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
pub use error::{IntoKernelReport, KernelError, KernelResult};

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
pub use storage::{ObjectStore, Storage};

// RAG traits (向量存储接口)
pub mod rag;
pub use rag::{
    Document, DocumentChunk, GenerateInput, Generator, RagPipeline, RagPipelineOutput, Reranker,
    Retriever, ScoredDocument, SearchResult, SimilarityMetric, VectorStore,
};

// Workflow traits (工作流接口)
pub mod workflow;
pub use workflow::*;
pub mod llm;
// Metrics traits for monitoring integration
pub mod metrics;
pub use metrics::*;

// Structured output parsing with JSON schema validation
pub mod structured_output;
pub use structured_output::StructuredOutput;
// Security governance (PII redaction, content moderation, prompt guard)
pub mod security;

// Gateway routing abstractions (kernel-level traits for agent request dispatch)
pub mod gateway;
pub use gateway::{GatewayRoute, HttpMethod, RegistryError, RouteRegistry, RoutingContext};

// Scheduler kernel contract (traits, types, errors for periodic agent execution)
pub mod scheduler;
pub use scheduler::{
    AgentScheduler, Clock, MissedTickPolicy, ScheduleDefinition,
    ScheduleHandle, ScheduleInfo, SchedulerError,
};
