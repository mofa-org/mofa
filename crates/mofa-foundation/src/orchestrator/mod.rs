//! Model Orchestrator Module
//!
//! This module provides edge model orchestration capabilities including:
//! - Model type routing (ASR / LLM / TTS / Embedding)
//! - Model lifecycle management (load/unload with idle timeout)
//! - Smart scheduling with LRU eviction
//! - Memory pressure awareness and dynamic precision degradation
//! - Multi-model pipeline chaining (ASR → LLM → TTS)
//!
//! ## GSoC 2026 \"Edge Model Orchestrator\" — Idea 3
//!
//! - **Lifecycle Management**: Automatic loading/unloading with configurable idle timeout
//! - **Smart Scheduling**: LRU-based eviction when memory is constrained
//! - **Pipeline Chaining**: Zero-copy ASR→LLM→TTS pipeline with per-stage latency tracking
//! - **Linux Integration**: Native Candle inference with CUDA/CPU device selection
//! - **Degradation Strategy**: Auto precision reduction (F32→F16→Q8→Q4) under pressure

pub mod traits;

#[cfg(all(target_os = "linux", feature = "linux-candle"))]
pub mod linux_candle;

#[cfg(all(target_os = "linux", feature = "linux-candle"))]
pub mod pipeline;

// Re-export core traits and types
pub use traits::{
    DegradationLevel, ModelOrchestrator, ModelProvider, ModelProviderConfig, ModelType,
    OrchestratorError, OrchestratorResult, PoolStatistics,
};

// Re-export Linux implementation when available
#[cfg(all(target_os = "linux", feature = "linux-candle"))]
pub use linux_candle::{LinuxCandleProvider, ModelPool};

// Re-export pipeline types when available
#[cfg(all(target_os = "linux", feature = "linux-candle"))]
pub use pipeline::{InferencePipeline, PipelineBuilder, PipelineOutput, PipelineStage};
