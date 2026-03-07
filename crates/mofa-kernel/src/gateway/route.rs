//! Core routing types and registry trait for agent request dispatch.
//!
//! [`GatewayRoute`] describes a single routing rule that maps an incoming
//! HTTP path + method to a target agent.  [`RouteRegistry`] is the
//! kernel-level trait any registry implementation must satisfy so that
//! routing logic can be tested and extended without depending on the full
//! HTTP stack.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use super::error::RegistryError;

// ─────────────────────────────────────────────────────────────────────────────
// HTTP method
// ─────────────────────────────────────────────────────────────────────────────

/// Standard HTTP methods accepted by a gateway route.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[non_exhaustive]
pub enum HttpMethod {
    Get,
    Post,
    Put,
    Patch,
    Delete,
    Head,
    Options,
}

impl HttpMethod {
    /// Case-insensitive parse from a string slice.  Returns `None` for unknown
    /// method strings.
    pub fn from_str_ci(s: &str) -> Option<Self> {
        match s.to_ascii_uppercase().as_str() {
            "GET" => Some(Self::Get),
            "POST" => Some(Self::Post),
            "PUT" => Some(Self::Put),
            "PATCH" => Some(Self::Patch),
            "DELETE" => Some(Self::Delete),
            "HEAD" => Some(Self::Head),
            "OPTIONS" => Some(Self::Options),
            _ => None,
        }
    }

    /// Return the standard uppercase string representation.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Get => "GET",
            Self::Post => "POST",
            Self::Put => "PUT",
            Self::Patch => "PATCH",
            Self::Delete => "DELETE",
            Self::Head => "HEAD",
            Self::Options => "OPTIONS",
        }
    }
}

impl std::fmt::Display for HttpMethod {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// GatewayRoute
// ─────────────────────────────────────────────────────────────────────────────

/// A single routing rule mapping an incoming HTTP path + method to a target
/// agent.
///
/// Routes can be toggled at runtime via the [`enabled`](GatewayRoute::enabled)
/// flag without having to deregister and re-register them.  This allows
/// operators to disable a route during maintenance without losing its
/// configuration.
///
/// # Priority
///
/// When multiple routes match the same `(path_pattern, method)` pair, the
/// route with the **highest** `priority` value wins.  Two routes that share
/// the same path, method, *and* priority value represent a conflict; see
/// [`RouteRegistry::register`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GatewayRoute {
    /// Stable, unique identifier for this route.
    pub id: String,
    /// ID of the agent that will handle matching requests.
    pub agent_id: String,
    /// URL path pattern, e.g. `/agents/summarizer` or `/v1/models/{model_id}`.
    /// Must begin with `/`.
    pub path_pattern: String,
    /// HTTP method this route accepts.
    pub method: HttpMethod,
    /// Numeric priority — higher values are evaluated first.
    /// Defaults to `0`.
    pub priority: i32,
    /// Whether this route is currently active.  Disabled routes are never
    /// returned by [`RouteRegistry::list_active`] and never matched at
    /// dispatch time.
    pub enabled: bool,
}

impl GatewayRoute {
    /// Create a new, enabled route with default priority.
    pub fn new(
        id: impl Into<String>,
        agent_id: impl Into<String>,
        path_pattern: impl Into<String>,
        method: HttpMethod,
    ) -> Self {
        Self {
            id: id.into(),
            agent_id: agent_id.into(),
            path_pattern: path_pattern.into(),
            method,
            priority: 0,
            enabled: true,
        }
    }

    /// Set the routing priority.
    pub fn with_priority(mut self, priority: i32) -> Self {
        self.priority = priority;
        self
    }

    /// Mark this route as disabled at creation time.
    pub fn disabled(mut self) -> Self {
        self.enabled = false;
        self
    }

    /// Validate that mandatory fields are well-formed.
    pub fn validate(&self) -> Result<(), RegistryError> {
        if self.id.trim().is_empty() {
            return Err(RegistryError::EmptyRouteId);
        }
        if self.agent_id.trim().is_empty() {
            return Err(RegistryError::EmptyAgentId);
        }
        if self.path_pattern.trim().is_empty() || !self.path_pattern.starts_with('/') {
            return Err(RegistryError::InvalidPathPattern(
                self.id.clone(),
                "path pattern must be non-empty and start with '/'".to_string(),
            ));
        }
        Ok(())
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// RouteRegistry trait
// ─────────────────────────────────────────────────────────────────────────────

/// Kernel contract for a route registry.
///
/// Implementations must be `Send + Sync` so they can be wrapped in an `Arc`
/// and shared across Tokio tasks without additional synchronisation.
///
/// # Conflict policy
///
/// Registering two routes with the same `(path_pattern, method)` pair *and*
/// the same `priority` is an error ([`RegistryError::ConflictingRoutes`]).
/// Routes that share the same pattern and method but differ in priority are
/// accepted; the higher-priority route wins at dispatch time.
pub trait RouteRegistry: Send + Sync {
    /// Register a new route.
    ///
    /// # Errors
    ///
    /// * [`RegistryError::DuplicateRouteId`] — a route with the same `id` is
    ///   already registered.
    /// * [`RegistryError::ConflictingRoutes`] — another route with the same
    ///   `(path_pattern, method, priority)` triple already exists.
    /// * [`RegistryError::InvalidRoute`] — the route fails field validation.
    fn register(&mut self, route: GatewayRoute) -> Result<(), RegistryError>;

    /// Remove a previously registered route by its ID.
    ///
    /// # Errors
    ///
    /// * [`RegistryError::RouteNotFound`] — no route with the given `id` is
    ///   registered.
    fn deregister(&mut self, route_id: &str) -> Result<(), RegistryError>;

    /// Look up a route by its stable ID, regardless of whether it is enabled.
    ///
    /// Returns `None` when no route with the given `id` is registered.
    fn lookup(&self, route_id: &str) -> Option<&GatewayRoute>;

    /// Return references to all **enabled** routes, sorted by descending
    /// priority (highest first).
    fn list_active(&self) -> Vec<&GatewayRoute>;
}

// ─────────────────────────────────────────────────────────────────────────────
// RoutingContext
// ─────────────────────────────────────────────────────────────────────────────

/// Information available at dispatch time, passed to routing strategies so
/// they can make decisions without depending on the raw HTTP request type.
///
/// The `correlation_id` field is a per-request identifier that flows through
/// the entire system for distributed tracing and log correlation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RoutingContext {
    /// Incoming request path, e.g. `/agents/summarizer`.
    pub path: String,
    /// HTTP method of the incoming request.
    pub method: HttpMethod,
    /// Parsed request headers.  Keys are lowercased for case-insensitive
    /// lookup (e.g. `"content-type"`).
    pub headers: HashMap<String, String>,
    /// Per-request correlation identifier for distributed tracing.
    pub correlation_id: String,
}

impl RoutingContext {
    /// Create a minimal context with no headers.
    pub fn new(
        path: impl Into<String>,
        method: HttpMethod,
        correlation_id: impl Into<String>,
    ) -> Self {
        Self {
            path: path.into(),
            method,
            headers: HashMap::new(),
            correlation_id: correlation_id.into(),
        }
    }

    /// Builder: insert a header (key is lowercased automatically).
    pub fn with_header(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.headers.insert(key.into().to_lowercase(), value.into());
        self
    }
}
