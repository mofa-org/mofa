//! Gateway error types for `mofa-kernel`.
//!
//! [`GatewayError`] covers every failure mode that can be detected at
//! *definition time* — empty IDs, duplicate registrations, missing backend
//! references, invalid configuration values — before any network I/O occurs.
//! Runtime failures (connection refused, upstream timeout, …) belong in the
//! gateway implementation crate (`mofa-gateway`).

use thiserror::Error;

/// Compile-time / configuration error type for the gateway kernel contract.
///
/// All variants are `#[non_exhaustive]` at the enum level so future releases
/// can add new failure modes without breaking existing `match` arms.
#[derive(Debug, Error, PartialEq, Eq)]
#[non_exhaustive]
pub enum GatewayError {
    // ── Identity ────────────────────────────────────────────────────────────
    /// The gateway configuration `id` field is empty or whitespace-only.
    #[error("gateway id cannot be empty")]
    EmptyGatewayId,

    // ── Routes ───────────────────────────────────────────────────────────────
    /// The configuration contains no routes.
    #[error("gateway config must define at least one route")]
    NoRoutes,

    /// A route `id` field is empty or whitespace-only.
    #[error("route id cannot be empty")]
    EmptyRouteId,

    /// A route with this id has already been registered.
    #[error("route '{0}' is already registered")]
    DuplicateRoute(String),

    /// No route with this id is currently registered.
    #[error("route '{0}' is not registered")]
    RouteNotFound(String),

    /// A route references a backend id that is not present in the backend list.
    #[error("route '{0}' references unknown backend '{1}'")]
    UnknownBackend(String, String),

    /// A route path pattern is syntactically invalid.
    #[error("route '{0}' has an invalid path pattern: {1}")]
    InvalidPathPattern(String, String),

    // ── Backends ─────────────────────────────────────────────────────────────
    /// The configuration contains no backends.
    #[error("gateway config must define at least one backend")]
    NoBackends,

    /// A backend `id` field is empty or whitespace-only.
    #[error("backend id cannot be empty")]
    EmptyBackendId,

    /// A backend with this id has already been registered.
    #[error("backend '{0}' is already registered")]
    DuplicateBackend(String),

    /// No backend with this id is currently registered.
    #[error("backend '{0}' is not registered")]
    BackendNotFound(String),

    /// A backend endpoint URI is syntactically invalid.
    #[error("backend '{0}' has an invalid endpoint URI: {1}")]
    InvalidEndpoint(String, String),

    // ── Filters ──────────────────────────────────────────────────────────────
    /// A filter chain is empty (must contain at least one filter).
    #[error("filter chain must contain at least one filter")]
    EmptyFilterChain,

    /// A filter priority / order value is invalid.
    #[error("filter has an invalid order value: {0}")]
    InvalidFilterOrder(String),

    // ── Auth ─────────────────────────────────────────────────────────────────
    /// An authentication configuration block is missing a required field.
    #[error("authentication config is missing required field: {0}")]
    InvalidAuthConfig(String),

    // ── Timeouts / rate-limits ────────────────────────────────────────────────
    /// `request_timeout_ms` is zero, which would reject every request.
    #[error("request timeout must be greater than 0 ms")]
    InvalidTimeout,

    /// The burst capacity is smaller than the sustained rate — nonsensical.
    #[error("rate limit burst capacity must be >= sustained rate per second")]
    InvalidRateLimit,
}
