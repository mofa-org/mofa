//! Shared application state for the control-plane server

use crate::middleware::RateLimiter;
use mofa_runtime::agent::registry::AgentRegistry;
use std::sync::Arc;

/// State shared across all request handlers
#[derive(Clone)]
pub struct AppState {
    /// Agent registry - source of truth for all running agents
    pub registry: Arc<AgentRegistry>,
    /// Per-client rate limiter
    pub rate_limiter: Arc<RateLimiter>,
}

impl AppState {
    /// Create a new `AppState` wrapping the given `AgentRegistry`.
    pub fn new(registry: Arc<AgentRegistry>, rate_limiter: Arc<RateLimiter>) -> Self {
        Self {
            registry,
            rate_limiter,
        }
    }
}
