//! Metrics traits and types for monitoring integration
//!
//! This module provides abstractions for connecting the monitoring layer
//! to various data sources without creating circular dependencies.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

/// Summary statistics for LLM API calls
///
/// This struct represents aggregated metrics from LLM API calls,
/// typically retrieved from a persistence layer.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LLMStatsSummary {
    /// Total number of requests
    pub total_requests: u64,
    /// Number of successful requests
    pub successful_requests: u64,
    /// Number of failed requests
    pub failed_requests: u64,
    /// Total tokens used (prompt + completion)
    pub total_tokens: u64,
    /// Total prompt tokens
    pub prompt_tokens: u64,
    /// Total completion tokens
    pub completion_tokens: u64,
    /// Average latency in milliseconds
    pub avg_latency_ms: f64,
    /// Average tokens per second (for streaming)
    pub tokens_per_second: Option<f64>,
}

/// Trait for providing LLM metrics to the monitoring layer
///
/// Implement this trait to connect a metrics data source (like persistence)
/// to the monitoring collector. This allows the monitoring layer to pull
/// LLM statistics without needing direct dependencies on persistence crates.
#[async_trait]
pub trait LLMMetricsSource: Send + Sync {
    /// Fetch aggregated LLM statistics from the underlying source
    ///
    /// Returns a summary of all LLM API calls stored in the source.
    /// The implementation should handle aggregation of individual call records
    /// into summary statistics.
    async fn get_llm_statistics(&self) -> Result<LLMStatsSummary, Box<dyn std::error::Error + Send + Sync>>;
}
