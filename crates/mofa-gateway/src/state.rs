//! Shared application state for the control-plane server

use crate::inference_bridge::InferenceBridge;
use crate::middleware::RateLimiter;
use mofa_foundation::inference::OrchestratorConfig;
use mofa_runtime::agent::registry::AgentRegistry;
use std::sync::Arc;

/// State shared across all request handlers
#[derive(Clone)]
pub struct AppState {
    /// Agent registry - source of truth for all running agents
    pub registry: Arc<AgentRegistry>,
    /// Per-client rate limiter
    pub rate_limiter: Arc<RateLimiter>,
    /// Inference bridge - connects to InferenceOrchestrator (optional)
    pub inference_bridge: Option<Arc<InferenceBridge>>,
}

impl AppState {
    /// Create a new `AppState` wrapping the given `AgentRegistry`.
    pub fn new(registry: Arc<AgentRegistry>, rate_limiter: Arc<RateLimiter>) -> Self {
        Self {
            registry,
            rate_limiter,
            inference_bridge: None,
        }
    }

    /// Create a new `AppState` with an inference bridge.
    pub fn with_inference_bridge(
        registry: Arc<AgentRegistry>,
        rate_limiter: Arc<RateLimiter>,
        orchestrator_config: OrchestratorConfig,
    ) -> Self {
        let bridge = InferenceBridge::new(orchestrator_config);
        Self {
            registry,
            rate_limiter,
            inference_bridge: Some(Arc::new(bridge)),
        }
    }
}
