//! Model Orchestrator Module
//!
//! This module provides edge model orchestration capabilities including:
//! - Model lifecycle management (load/unload)
//! - Smart scheduling with LRU eviction
//! - Memory pressure awareness
//! - Multi-model coordination
//!
//! ## GSoC 2026 "Edge Model Orchestrator" Implementation
//!
//! This module implements the requirements for Idea 3:
//! - **Lifecycle Management**: Automatic model loading/unloading with idle timeout
//! - **Smart Scheduling**: LRU-based eviction when memory is constrained
//! - **Linux Integration**: Native Candle inference with CUDA/CPU device selection

pub mod traits;

#[cfg(target_os = "linux")]
pub mod linux_candle;

// Re-export core traits and types
pub use traits::{
    ModelOrchestrator, ModelProvider, ModelProviderConfig, OrchestratorError, OrchestratorResult,
    PoolStatistics,
};

// Re-export Linux implementation when available
#[cfg(target_os = "linux")]
pub use linux_candle::{LinuxCandleProvider, ModelPool};
