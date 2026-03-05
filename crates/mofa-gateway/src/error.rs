//! `mofa-gateway` implementation error type.
//!
//! [`GatewayImplError`] covers *runtime* failures that occur after
//! configuration has been validated: network errors, upstream timeouts,
//! serialisation failures during proxying, etc.
//!
//! Configuration failures (wrong IDs, duplicate routes, …) are represented
//! by [`mofa_kernel::gateway::GatewayError`] and live in the kernel crate.

use thiserror::Error;

/// Runtime error type for `mofa-gateway`.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum GatewayImplError {
    /// The upstream backend returned an HTTP error status.
    #[error("upstream '{backend_id}' returned HTTP {status}: {message}")]
    UpstreamError {
        backend_id: String,
        status: u16,
        message: String,
    },

    /// A network-level error communicating with the upstream backend.
    #[error("upstream '{backend_id}' network error: {source}")]
    NetworkError {
        backend_id: String,
        #[source]
        source: reqwest::Error,
    },

    /// No alive backend could be found for the requested route.
    #[error("no healthy backend available for route '{route_id}'")]
    NoHealthyBackend { route_id: String },

    /// The inbound request could not be routed (no matching route).
    #[error("no route matched path '{path}' method '{method}'")]
    RoutingFailure { path: String, method: String },

    /// A filter in the chain returned an unrecoverable error.
    #[error("filter '{filter_name}' failed: {message}")]
    FilterError {
        filter_name: String,
        message: String,
    },

    /// JSON (de)serialisation error inside the proxy.
    #[error("serialisation error: {0}")]
    Serialization(#[from] serde_json::Error),

    /// Generic internal error with a human-readable message.
    #[error("internal gateway error: {0}")]
    Internal(String),
}

/// Convenience alias.
pub type GatewayResult<T> = Result<T, GatewayImplError>;
