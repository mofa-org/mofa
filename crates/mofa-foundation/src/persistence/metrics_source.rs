//! Metrics source implementation for persistence layer
//!
//! Provides an implementation of the LLMMetricsSource trait
//! that retrieves metrics from an ApiCallStore.

use mofa_kernel::metrics::{LLMMetricsSource, LLMStatsSummary};
use crate::persistence::{DynApiCallStore, QueryFilter};
use async_trait::async_trait;
use std::sync::Arc;

/// Metrics source that retrieves LLM statistics from persistence storage
///
/// This implementation queries the ApiCallStore to retrieve aggregated
/// statistics about LLM API calls. It can be used with any backend
/// that implements the ApiCallStore trait (Memory, PostgreSQL, MySQL, SQLite).
pub struct PersistenceMetricsSource {
    store: DynApiCallStore,
}

impl PersistenceMetricsSource {
    /// Create a new PersistenceMetricsSource
    pub fn new(store: DynApiCallStore) -> Self {
        Self { store }
    }
}

#[async_trait]
impl LLMMetricsSource for PersistenceMetricsSource {
    async fn get_llm_statistics(&self) -> Result<LLMStatsSummary, Box<dyn std::error::Error + Send + Sync>> {
        let stats = self.store.get_statistics(&QueryFilter::default()).await?;

        Ok(LLMStatsSummary {
            total_requests: stats.total_calls as u64,
            successful_requests: stats.success_count as u64,
            failed_requests: stats.failed_count as u64,
            total_tokens: stats.total_tokens as u64,
            prompt_tokens: stats.total_prompt_tokens as u64,
            completion_tokens: stats.total_completion_tokens as u64,
            avg_latency_ms: stats.avg_latency_ms.unwrap_or(0.0),
            tokens_per_second: stats.avg_tokens_per_second,
        })
    }
}
