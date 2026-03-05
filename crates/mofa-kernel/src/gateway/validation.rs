//! Gateway configuration container and compile-time validation.
//!
//! [`GatewayConfig`] aggregates the three configuration dimensions
//! (routes, backends, global settings) and exposes a single [`validate()`]
//! method that checks all structural invariants *before* any runtime
//! resources are allocated.
//!
//! This mirrors the `MessageGraph::validate()` / `MessageGraph::compile()`
//! pattern established in `mofa-kernel::message_graph`.

use super::capability::CapabilityDescriptor;
use super::error::GatewayError;
use super::filter::FilterChainConfig;
use super::router::RouteConfig;
use std::collections::HashSet;

// ─────────────────────────────────────────────────────────────────────────────
// RateLimitConfig
// ─────────────────────────────────────────────────────────────────────────────

/// Simple token-bucket rate-limit parameters.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RateLimitConfig {
    /// Sustained token refill rate (tokens per second).
    pub rate_per_second: u32,
    /// Maximum burst capacity (must be >= `rate_per_second`).
    pub burst_capacity: u32,
}

impl RateLimitConfig {
    /// Create a new rate-limit config.
    pub fn new(rate_per_second: u32, burst_capacity: u32) -> Self {
        Self {
            rate_per_second,
            burst_capacity,
        }
    }

    fn validate(&self) -> Result<(), GatewayError> {
        if self.burst_capacity < self.rate_per_second {
            return Err(GatewayError::InvalidRateLimit);
        }
        Ok(())
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// GatewayConfig
// ─────────────────────────────────────────────────────────────────────────────

/// Top-level gateway configuration.
///
/// Call [`validate()`](Self::validate) to check all structural invariants
/// before passing this config to the gateway runtime.
#[derive(Debug, Clone)]
pub struct GatewayConfig {
    /// Unique identifier for this gateway instance.
    pub id: String,
    /// All route definitions.
    pub routes: Vec<RouteConfig>,
    /// All registered backend descriptors.
    pub backends: Vec<CapabilityDescriptor>,
    /// Optional filter chain configuration.
    pub filter_chain: Option<FilterChainConfig>,
    /// Global default request timeout in milliseconds (must be > 0).
    pub request_timeout_ms: u64,
    /// Optional global rate-limit configuration.
    pub rate_limit: Option<RateLimitConfig>,
}

impl GatewayConfig {
    /// Construct a minimal config with only a gateway id.
    pub fn new(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            routes: Vec::new(),
            backends: Vec::new(),
            filter_chain: None,
            request_timeout_ms: 30_000,
            rate_limit: None,
        }
    }

    /// Builder: add a route.
    pub fn with_route(mut self, route: RouteConfig) -> Self {
        self.routes.push(route);
        self
    }

    /// Builder: add a backend.
    pub fn with_backend(mut self, backend: CapabilityDescriptor) -> Self {
        self.backends.push(backend);
        self
    }

    /// Builder: set the filter chain.
    pub fn with_filter_chain(mut self, chain: FilterChainConfig) -> Self {
        self.filter_chain = Some(chain);
        self
    }

    /// Builder: set the global request timeout.
    pub fn with_timeout_ms(mut self, ms: u64) -> Self {
        self.request_timeout_ms = ms;
        self
    }

    /// Builder: set the rate-limit config.
    pub fn with_rate_limit(mut self, rl: RateLimitConfig) -> Self {
        self.rate_limit = Some(rl);
        self
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Validation
    // ─────────────────────────────────────────────────────────────────────────

    /// Validate all structural invariants of this configuration.
    ///
    /// Returns `Ok(())` if the configuration is structurally sound and can be
    /// used to initialise the gateway runtime.  Returns the *first* detected
    /// [`GatewayError`] otherwise.
    ///
    /// Checks performed (in order):
    /// 1. Gateway id is non-empty.
    /// 2. At least one route is defined.
    /// 3. At least one backend is defined.
    /// 4. Global `request_timeout_ms` is non-zero.
    /// 5. Each backend passes its own [`CapabilityDescriptor::validate()`] check.
    /// 6. No two backends share the same id.
    /// 7. Each route passes its own [`RouteConfig::validate()`] check.
    /// 8. No two routes share the same id.
    /// 9. Every route's `backend_id` refers to a declared backend.
    /// 10. If a filter chain is present, it is non-empty.
    /// 11. If a rate-limit config is present, burst >= rate.
    pub fn validate(&self) -> Result<(), GatewayError> {
        // ── 1. Gateway id ────────────────────────────────────────────────────
        if self.id.trim().is_empty() {
            return Err(GatewayError::EmptyGatewayId);
        }

        // ── 2. At least one route ────────────────────────────────────────────
        if self.routes.is_empty() {
            return Err(GatewayError::NoRoutes);
        }

        // ── 3. At least one backend ──────────────────────────────────────────
        if self.backends.is_empty() {
            return Err(GatewayError::NoBackends);
        }

        // ── 4. Global timeout is non-zero ────────────────────────────────────
        if self.request_timeout_ms == 0 {
            return Err(GatewayError::InvalidTimeout);
        }

        // ── Build backend id lookup set ───────────────────────────────────────
        let mut backend_ids: HashSet<&str> = HashSet::new();

        // ── 8 + 9. Validate each backend, check for duplicates ────────────────
        for backend in &self.backends {
            backend.validate()?;
            if !backend_ids.insert(backend.id.as_str()) {
                return Err(GatewayError::DuplicateBackend(backend.id.clone()));
            }
        }

        // ── 5 + 6 + 7. Validate each route ───────────────────────────────────
        let mut route_ids: HashSet<&str> = HashSet::new();
        for route in &self.routes {
            route.validate()?;
            if !route_ids.insert(route.id.as_str()) {
                return Err(GatewayError::DuplicateRoute(route.id.clone()));
            }
            if !backend_ids.contains(route.backend_id.as_str()) {
                return Err(GatewayError::UnknownBackend(
                    route.id.clone(),
                    route.backend_id.clone(),
                ));
            }
        }

        // ── 10. Filter chain must be non-empty if present ────────────────────
        if self
            .filter_chain
            .as_ref()
            .is_some_and(|chain| chain.filter_names.is_empty())
        {
            return Err(GatewayError::EmptyFilterChain);
        }

        // ── 11. Rate-limit burst >= rate ──────────────────────────────────────
        if let Some(rl) = &self.rate_limit {
            rl.validate()?;
        }

        Ok(())
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gateway::capability::{BackendKind, CapabilityDescriptor};
    use crate::gateway::filter::FilterChainConfig;
    use crate::gateway::router::RouteConfig;

    // ── Helpers ───────────────────────────────────────────────────────────────

    fn openai_backend() -> CapabilityDescriptor {
        CapabilityDescriptor::new("openai", BackendKind::LlmOpenAI, "https://api.openai.com")
    }

    fn chat_route() -> RouteConfig {
        RouteConfig::new("chat", "/v1/chat/completions", "openai")
    }

    fn valid_config() -> GatewayConfig {
        GatewayConfig::new("gateway-test")
            .with_backend(openai_backend())
            .with_route(chat_route())
    }

    // ── Happy path ────────────────────────────────────────────────────────────

    #[test]
    fn valid_config_passes_validation() {
        assert!(valid_config().validate().is_ok());
    }

    #[test]
    fn valid_config_with_filter_chain_passes() {
        let chain = FilterChainConfig::new("default", vec!["auth".to_string(), "log".to_string()]);
        let cfg = valid_config().with_filter_chain(chain);
        assert!(cfg.validate().is_ok());
    }

    #[test]
    fn valid_config_with_rate_limit_passes() {
        let rl = RateLimitConfig::new(100, 200);
        let cfg = valid_config().with_rate_limit(rl);
        assert!(cfg.validate().is_ok());
    }

    #[test]
    fn multiple_routes_and_backends_pass() {
        let anthropic =
            CapabilityDescriptor::new("anthropic", BackendKind::LlmAnthropic, "https://api.anthropic.com");
        let models_route = RouteConfig::new("models", "/v1/models", "anthropic");
        let cfg = valid_config()
            .with_backend(anthropic)
            .with_route(models_route);
        assert!(cfg.validate().is_ok());
    }

    // ── Identity errors ───────────────────────────────────────────────────────

    #[test]
    fn empty_gateway_id_returns_error() {
        let cfg = GatewayConfig::new("")
            .with_backend(openai_backend())
            .with_route(chat_route());
        assert_eq!(cfg.validate(), Err(GatewayError::EmptyGatewayId));
    }

    #[test]
    fn whitespace_only_gateway_id_returns_error() {
        let cfg = GatewayConfig::new("   ")
            .with_backend(openai_backend())
            .with_route(chat_route());
        assert_eq!(cfg.validate(), Err(GatewayError::EmptyGatewayId));
    }

    // ── Route errors ──────────────────────────────────────────────────────────

    #[test]
    fn no_routes_returns_error() {
        let cfg = GatewayConfig::new("gw").with_backend(openai_backend());
        assert_eq!(cfg.validate(), Err(GatewayError::NoRoutes));
    }

    #[test]
    fn duplicate_route_id_returns_error() {
        let cfg = GatewayConfig::new("gw")
            .with_backend(openai_backend())
            .with_route(chat_route())
            .with_route(chat_route()); // same id
        assert_eq!(
            cfg.validate(),
            Err(GatewayError::DuplicateRoute("chat".to_string()))
        );
    }

    #[test]
    fn route_with_empty_id_returns_error() {
        let bad_route = RouteConfig::new("", "/v1/chat/completions", "openai");
        let cfg = GatewayConfig::new("gw")
            .with_backend(openai_backend())
            .with_route(bad_route);
        assert_eq!(cfg.validate(), Err(GatewayError::EmptyRouteId));
    }

    #[test]
    fn route_path_missing_leading_slash_returns_error() {
        let bad_route = RouteConfig::new("chat", "v1/chat/completions", "openai");
        let cfg = GatewayConfig::new("gw")
            .with_backend(openai_backend())
            .with_route(bad_route);
        assert!(matches!(
            cfg.validate(),
            Err(GatewayError::InvalidPathPattern(ref id, _)) if id == "chat"
        ));
    }

    #[test]
    fn route_referencing_unknown_backend_returns_error() {
        let route = RouteConfig::new("chat", "/v1/chat/completions", "nonexistent-backend");
        let cfg = GatewayConfig::new("gw")
            .with_backend(openai_backend())
            .with_route(route);
        assert!(matches!(
            cfg.validate(),
            Err(GatewayError::UnknownBackend(ref rid, ref bid))
                if rid == "chat" && bid == "nonexistent-backend"
        ));
    }

    // ── Backend errors ────────────────────────────────────────────────────────

    #[test]
    fn no_backends_returns_error() {
        let cfg = GatewayConfig::new("gw").with_route(chat_route());
        assert_eq!(cfg.validate(), Err(GatewayError::NoBackends));
    }

    #[test]
    fn duplicate_backend_id_returns_error() {
        let cfg = GatewayConfig::new("gw")
            .with_backend(openai_backend())
            .with_backend(openai_backend()) // same id
            .with_route(chat_route());
        assert_eq!(
            cfg.validate(),
            Err(GatewayError::DuplicateBackend("openai".to_string()))
        );
    }

    #[test]
    fn backend_with_empty_id_returns_error() {
        let bad = CapabilityDescriptor::new("", BackendKind::LlmOpenAI, "https://api.openai.com");
        let cfg = GatewayConfig::new("gw")
            .with_backend(bad)
            .with_route(chat_route());
        assert_eq!(cfg.validate(), Err(GatewayError::EmptyBackendId));
    }

    #[test]
    fn backend_with_empty_endpoint_returns_error() {
        let bad = CapabilityDescriptor::new("openai", BackendKind::LlmOpenAI, "");
        let cfg = GatewayConfig::new("gw")
            .with_backend(bad)
            .with_route(chat_route());
        assert!(matches!(
            cfg.validate(),
            Err(GatewayError::InvalidEndpoint(ref id, _)) if id == "openai"
        ));
    }

    #[test]
    fn backend_endpoint_without_http_scheme_returns_error() {
        let bad = CapabilityDescriptor::new("openai", BackendKind::LlmOpenAI, "ftp://badscheme.com");
        let cfg = GatewayConfig::new("gw")
            .with_backend(bad)
            .with_route(chat_route());
        assert!(matches!(
            cfg.validate(),
            Err(GatewayError::InvalidEndpoint(ref id, _)) if id == "openai"
        ));
    }

    // ── Timeout errors ────────────────────────────────────────────────────────

    #[test]
    fn zero_request_timeout_returns_error() {
        let cfg = valid_config().with_timeout_ms(0);
        assert_eq!(cfg.validate(), Err(GatewayError::InvalidTimeout));
    }

    // ── Filter chain errors ───────────────────────────────────────────────────

    #[test]
    fn empty_filter_chain_returns_error() {
        let chain = FilterChainConfig::new("default", vec![]);
        let cfg = valid_config().with_filter_chain(chain);
        assert_eq!(cfg.validate(), Err(GatewayError::EmptyFilterChain));
    }

    // ── Rate limit errors ─────────────────────────────────────────────────────

    #[test]
    fn burst_less_than_rate_returns_error() {
        let rl = RateLimitConfig::new(100, 50); // burst < rate — invalid
        let cfg = valid_config().with_rate_limit(rl);
        assert_eq!(cfg.validate(), Err(GatewayError::InvalidRateLimit));
    }

    #[test]
    fn burst_equal_to_rate_passes() {
        let rl = RateLimitConfig::new(100, 100); // burst == rate — valid
        let cfg = valid_config().with_rate_limit(rl);
        assert!(cfg.validate().is_ok());
    }
}
