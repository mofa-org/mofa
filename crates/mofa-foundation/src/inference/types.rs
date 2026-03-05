//! Shared types for the unified inference orchestration layer.
//!
//! Re-exported from mofa_kernel.

pub use mofa_kernel::llm::{
    InferenceRequest, InferenceResult, Precision, RequestPriority, RoutedBackend,
};
