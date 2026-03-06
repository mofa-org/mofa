//! Error types for the gateway routing layer.

use thiserror::Error;

/// Errors that can occur when interacting with a [`RouteRegistry`](super::route::RouteRegistry).
#[derive(Debug, Error, PartialEq, Eq)]
#[non_exhaustive]
pub enum RegistryError {
    /// The route `id` field is empty or whitespace-only.
    #[error("route id cannot be empty")]
    EmptyRouteId,

    /// The route `agent_id` field is empty or whitespace-only.
    #[error("agent id cannot be empty")]
    EmptyAgentId,

    /// The path pattern is invalid (empty, or does not start with `/`).
    #[error("route '{0}' has an invalid path pattern: {1}")]
    InvalidPathPattern(String, String),

    /// A route with this ID is already registered.
    #[error("route '{0}' is already registered")]
    DuplicateRouteId(String),

    /// No route with this ID is currently registered.
    #[error("route '{0}' not found")]
    RouteNotFound(String),

    /// Two routes share the same path pattern, method, and priority — the
    /// gateway cannot deterministically choose between them.
    #[error(
        "route '{0}' conflicts with existing route '{1}': \
         same path pattern, method, and priority"
    )]
    ConflictingRoutes(String, String),

    /// The route failed basic field validation.
    #[error("route is invalid: {0}")]
    InvalidRoute(String),
}
