//! Graceful degradation strategies invoked after retries are exhausted.

use async_trait::async_trait;
use mofa_kernel::agent::{error::AgentError, types::AgentOutput};

/// Called when all retry attempts fail. Return `Some(output)` to substitute a
/// degraded response, or `None` to propagate the error.
#[async_trait]
pub trait FallbackStrategy: Send + Sync {
    async fn on_failure(
        &self,
        agent_id: &str,
        error: &AgentError,
        attempt_count: usize,
    ) -> Option<AgentOutput>;
}

/// Returns a fixed static output useful for "service unavailable" placeholders.
pub struct StaticFallback {
    pub output: AgentOutput,
}

#[async_trait]
impl FallbackStrategy for StaticFallback {
    async fn on_failure(&self, _: &str, _: &AgentError, _: usize) -> Option<AgentOutput> {
        Some(self.output.clone())
    }
}

/// Returns `None` error propagates unchanged. Default behaviour.
pub struct NoFallback;

#[async_trait]
impl FallbackStrategy for NoFallback {
    async fn on_failure(&self, _: &str, _: &AgentError, _: usize) -> Option<AgentOutput> {
        None
    }
}
