//! Gateway router trait and configuration types.
//!
//! The [`GatewayRouter`] trait is the single kernel-level abstraction for
//! request routing.  Implementations (e.g. a trie-based router in
//! `mofa-gateway`) are registered against routes at startup and looked up
//! on every inbound request.

use super::config_error::GatewayConfigError;
use super::route::HttpMethod;
use super::types::RouteMatch;
use serde::{Deserialize, Serialize};

// ─────────────────────────────────────────────────────────────────────────────
// Route configuration
// ─────────────────────────────────────────────────────────────────────────────

/// A single routing rule mapping a path pattern + method set to a backend.
///
/// Path patterns follow the `{param}` template syntax used by axum 0.8+:
/// ```text
/// /v1/chat/completions          — exact path
/// /v1/models/{model_id}         — captures `model_id`
/// /v1/agents/{agent_id}/invoke  — captures `agent_id`
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RouteConfig {
    /// Unique stable identifier for this route.
    pub id: String,
    /// URL path template.  Must begin with `/`.
    pub path_pattern: String,
    /// Accepted HTTP methods.  An empty vec means *all* methods are accepted.
    pub methods: Vec<HttpMethod>,
    /// Id of the backend this route forwards to.
    pub backend_id: String,
    /// Per-route request timeout in milliseconds (overrides gateway default).
    /// A value of `0` means "use the gateway default".
    pub timeout_ms: u64,
    /// Routing priority: higher values are evaluated first when multiple
    /// patterns match the same path.
    pub priority: i32,
}

impl RouteConfig {
    /// Create a minimal route with just id, path_pattern, and backend_id.
    pub fn new(
        id: impl Into<String>,
        path_pattern: impl Into<String>,
        backend_id: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            path_pattern: path_pattern.into(),
            methods: Vec::new(),
            backend_id: backend_id.into(),
            timeout_ms: 0,
            priority: 0,
        }
    }

    /// Builder: restrict to specific HTTP methods.
    pub fn with_methods(mut self, methods: Vec<HttpMethod>) -> Self {
        self.methods = methods;
        self
    }

    /// Builder: set a per-route timeout.
    pub fn with_timeout_ms(mut self, ms: u64) -> Self {
        self.timeout_ms = ms;
        self
    }

    /// Builder: set routing priority (higher = evaluated first).
    pub fn with_priority(mut self, priority: i32) -> Self {
        self.priority = priority;
        self
    }

    /// Basic sanity checks run during gateway config validation.
    pub(crate) fn validate(&self) -> Result<(), GatewayConfigError> {
        if self.id.trim().is_empty() {
            return Err(GatewayConfigError::EmptyRouteId);
        }
        if self.path_pattern.trim().is_empty() {
            return Err(GatewayConfigError::InvalidPathPattern(
                self.id.clone(),
                "path pattern cannot be empty".to_string(),
            ));
        }
        if !self.path_pattern.starts_with('/') {
            return Err(GatewayConfigError::InvalidPathPattern(
                self.id.clone(),
                "path pattern must start with '/'".to_string(),
            ));
        }
        Ok(())
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Router trait
// ─────────────────────────────────────────────────────────────────────────────

/// Kernel contract for request routing.
///
/// Implementations receive [`RouteConfig`] entries at startup (via
/// [`register`](GatewayRouter::register)) and resolve incoming
/// (path, method) pairs to a [`RouteMatch`] at request time.
///
/// The trait is intentionally synchronous: route lookups must be O(depth)
/// in a trie — no I/O, no allocation on the hot path.
pub trait GatewayRouter: Send + Sync {
    /// Register a new route.  Returns [`GatewayConfigError::DuplicateRoute`] if
    /// a route with the same `id` is already registered.
    fn register(&mut self, route: RouteConfig) -> Result<(), GatewayConfigError>;

    /// Resolve a request `(path, method)` to the best matching route.
    /// Returns `None` when no route matches.
    fn resolve(&self, path: &str, method: &HttpMethod) -> Option<RouteMatch>;

    /// Return a snapshot of all registered routes, sorted by descending priority.
    fn routes(&self) -> Vec<&RouteConfig>;

    /// Remove a previously registered route.
    /// Returns [`GatewayConfigError::RouteNotFound`] if the id is not registered.
    fn deregister(&mut self, route_id: &str) -> Result<(), GatewayConfigError>;
}
