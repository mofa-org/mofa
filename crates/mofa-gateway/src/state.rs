//! Shared application state for the control-plane server

use crate::inference_bridge::InferenceBridge;
use crate::middleware::RateLimiter;
use mofa_foundation::inference::OrchestratorConfig;
use mofa_foundation::GatewayCapabilityRegistry;
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
    /// Shared gateway capability registry (optional)
    pub capability_registry: Option<Arc<GatewayCapabilityRegistry>>,
}

impl AppState {
    /// Create a new `AppState` wrapping the given `AgentRegistry`.
    pub fn new(registry: Arc<AgentRegistry>, rate_limiter: Arc<RateLimiter>) -> Self {
        Self {
            registry,
            rate_limiter,
            inference_bridge: None,
            capability_registry: None,
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
            capability_registry: None,
        }
    }

    /// Attach a capability registry to an existing app state.
    pub fn with_capability_registry(mut self, capability_registry: Arc<GatewayCapabilityRegistry>) -> Self {
        self.capability_registry = Some(capability_registry);
        self
    }
}
