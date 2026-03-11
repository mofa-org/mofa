//! Error types for the control plane and gateway.
//!
//! This module provides comprehensive error handling following MoFA's error
//! handling standards: using `thiserror` with `#[non_exhaustive]` enums for
//! API stability.

use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::Serialize;
use thiserror::Error;

/// Errors that can occur in control plane operations.
#[derive(Error, Debug)]
#[non_exhaustive]
pub enum ControlPlaneError {
    /// Error from the consensus engine.
    #[error("Consensus error: {0}")]
    Consensus(#[from] ConsensusError),

    /// Error in state machine operations.
    #[error("State machine error: {0}")]
    StateMachine(String),

    /// Error in cluster membership management.
    #[error("Cluster membership error: {0}")]
    Membership(String),

    /// Requested node was not found in the cluster.
    #[error("Node not found: {0}")]
    NodeNotFound(String),

    /// Operation requires leader role but this node is not the leader.
    #[error("Not the leader")]
    NotLeader,

    /// Leader election did not complete within the timeout period.
    #[error("Leader election timeout")]
    LeaderElectionTimeout,

    /// Network communication error.
    #[error("Network error: {0}")]
    Network(String),

    /// Storage operation error.
    #[error("Storage error: {0}")]
    Storage(String),

    /// Configuration error.
    #[error("Configuration error: {0}")]
    Config(String),

    /// Internal control plane error.
    #[error("Internal error: {0}")]
    Internal(String),
}

/// Errors that can occur in consensus operations.
#[derive(Error, Debug)]
#[non_exhaustive]
pub enum ConsensusError {
    /// Operation requires leader role but this node is not the leader.
    #[error("Not leader - current leader is {0}")]
    NotLeader(String),

    /// Failed to replicate log entries to followers.
    #[error("Log replication failed: {0}")]
    ReplicationFailed(String),

    /// Leader election process failed.
    #[error("Leader election failed: {0}")]
    ElectionFailed(String),

    /// Quorum (majority) of nodes not available for consensus.
    #[error("Quorum not reached: have {have}, need {need}")]
    QuorumNotReached {
        /// Number of nodes currently available.
        have: usize,
        /// Number of nodes required for quorum.
        need: usize,
    },

    /// Term number mismatch indicates stale request or split-brain scenario.
    #[error("Term mismatch: expected {expected}, got {got}")]
    TermMismatch {
        /// Expected term number.
        expected: u64,
        /// Actual term number received.
        got: u64,
    },

    /// Requested log entry does not exist at the given index.
    #[error("Log entry not found at index {0}")]
    LogEntryNotFound(u64),

    /// Network partition detected - cluster is split.
    #[error("Network partition detected")]
    NetworkPartition,

    /// Storage operation error.
    #[error("Storage error: {0}")]
    Storage(String),

    /// Internal consensus engine error.
    #[error("Internal consensus error: {0}")]
    Internal(String),
}

/// Errors that can occur in gateway operations.
#[derive(Error, Debug)]
#[non_exhaustive]
pub enum GatewayError {
    /// Invalid request parameters or payload.
    #[error("Invalid request: {0}")]
    InvalidRequest(String),

    /// Agent with the given ID already exists.
    #[error("Agent already exists: {0}")]
    AgentAlreadyExists(String),

    /// Agent operation failed (create, start, stop, etc.).
    #[error("Agent operation failed: {0}")]
    AgentOperationFailed(String),

    /// Agent not found in the registry.
    #[error("Agent not found: {0}")]
    AgentNotFound(String),

    /// Error in load balancing algorithm.
    #[error("Load balancing error: {0}")]
    LoadBalancing(String),

    /// Request rate limit exceeded.
    #[error("Rate limit exceeded: {0}")]
    RateLimitExceeded(String),

    /// Circuit breaker is open for the target node.
    #[error("Circuit breaker open: {0}")]
    CircuitBreakerOpen(String),

    /// No healthy nodes available for routing.
    #[error("No healthy nodes available: {0}")]
    NoHealthyNodes(String),

    /// Target node is marked as unhealthy.
    #[error("Unhealthy node: {0}")]
    UnhealthyNode(String),

    /// No nodes available in the load balancer.
    #[error("No available nodes: {0}")]
    NoAvailableNodes(String),

    /// Health check for a node failed.
    #[error("Node health check failed: {0}")]
    HealthCheckFailed(String),

    /// Request routing to backend failed.
    #[error("Request routing failed: {0}")]
    RoutingFailed(String),

    /// Operation timed out.
    #[error("Timeout: {0}")]
    Timeout(String),

    /// Network communication error.
    #[error("Network error: {0}")]
    Network(String),

    /// Internal gateway error.
    #[error("Internal gateway error: {0}")]
    Internal(String),
}

/// Result type for control plane operations.
pub type ControlPlaneResult<T> = Result<T, ControlPlaneError>;

/// Result type for consensus operations.
pub type ConsensusResult<T> = Result<T, ConsensusError>;

/// Result type for gateway operations.
pub type GatewayResult<T> = Result<T, GatewayError>;

// From impl is automatically generated by #[from] attribute above

impl From<std::io::Error> for ControlPlaneError {
    fn from(err: std::io::Error) -> Self {
        ControlPlaneError::Storage(err.to_string())
    }
}

// RocksDB error conversion is handled in storage module

// ─────────────────────────────────────────────────────────────────────────────
// Axum integration: convert GatewayError to HTTP responses
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Serialize)]
struct ErrorResponse {
    error: String,
}

impl IntoResponse for GatewayError {
    fn into_response(self) -> Response {
        let status = match self {
            GatewayError::InvalidRequest(_) => StatusCode::BAD_REQUEST,
            GatewayError::AgentAlreadyExists(_) => StatusCode::CONFLICT,
            GatewayError::AgentNotFound(_) => StatusCode::NOT_FOUND,
            GatewayError::AgentOperationFailed(_) => StatusCode::INTERNAL_SERVER_ERROR,
            GatewayError::RateLimitExceeded(_) => StatusCode::TOO_MANY_REQUESTS,
            GatewayError::CircuitBreakerOpen(_) => StatusCode::SERVICE_UNAVAILABLE,
            GatewayError::NoHealthyNodes(_) => StatusCode::SERVICE_UNAVAILABLE,
            GatewayError::UnhealthyNode(_) => StatusCode::SERVICE_UNAVAILABLE,
            GatewayError::NoAvailableNodes(_) => StatusCode::SERVICE_UNAVAILABLE,
            GatewayError::HealthCheckFailed(_) => StatusCode::SERVICE_UNAVAILABLE,
            GatewayError::RoutingFailed(_) => StatusCode::BAD_GATEWAY,
            GatewayError::Timeout(_) => StatusCode::REQUEST_TIMEOUT,
            GatewayError::Network(_) => StatusCode::BAD_GATEWAY,
            GatewayError::LoadBalancing(_) => StatusCode::INTERNAL_SERVER_ERROR,
            GatewayError::Internal(_) => StatusCode::INTERNAL_SERVER_ERROR,
        };

        let body = Json(ErrorResponse {
            error: self.to_string(),
        });

        (status, body).into_response()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::StatusCode;

    // --- ControlPlaneError Display ---

    #[test]
    fn control_plane_error_display() {
        let err = ControlPlaneError::StateMachine("bad state".into());
        assert_eq!(err.to_string(), "State machine error: bad state");

        let err = ControlPlaneError::NodeNotFound("node-1".into());
        assert_eq!(err.to_string(), "Node not found: node-1");

        let err = ControlPlaneError::NotLeader;
        assert_eq!(err.to_string(), "Not the leader");

        let err = ControlPlaneError::LeaderElectionTimeout;
        assert_eq!(err.to_string(), "Leader election timeout");
    }

    // --- ConsensusError Display ---

    #[test]
    fn consensus_error_display() {
        let err = ConsensusError::NotLeader("node-2".into());
        assert_eq!(err.to_string(), "Not leader - current leader is node-2");

        let err = ConsensusError::QuorumNotReached { have: 1, need: 3 };
        assert_eq!(
            err.to_string(),
            "Quorum not reached: have 1, need 3"
        );

        let err = ConsensusError::TermMismatch {
            expected: 5,
            got: 3,
        };
        assert_eq!(err.to_string(), "Term mismatch: expected 5, got 3");

        let err = ConsensusError::LogEntryNotFound(42);
        assert_eq!(err.to_string(), "Log entry not found at index 42");

        let err = ConsensusError::NetworkPartition;
        assert_eq!(err.to_string(), "Network partition detected");
    }

    // --- GatewayError Display ---

    #[test]
    fn gateway_error_display() {
        let err = GatewayError::InvalidRequest("missing field".into());
        assert_eq!(err.to_string(), "Invalid request: missing field");

        let err = GatewayError::AgentNotFound("agent-1".into());
        assert_eq!(err.to_string(), "Agent not found: agent-1");

        let err = GatewayError::RateLimitExceeded("10 req/s".into());
        assert_eq!(err.to_string(), "Rate limit exceeded: 10 req/s");

        let err = GatewayError::Timeout("30s".into());
        assert_eq!(err.to_string(), "Timeout: 30s");
    }

    // --- From conversions ---

    #[test]
    fn consensus_error_into_control_plane_error() {
        let consensus_err = ConsensusError::NetworkPartition;
        let cp_err: ControlPlaneError = consensus_err.into();
        assert!(matches!(cp_err, ControlPlaneError::Consensus(_)));
        assert!(cp_err.to_string().contains("Network partition detected"));
    }

    #[test]
    fn io_error_into_control_plane_error() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "file missing");
        let cp_err: ControlPlaneError = io_err.into();
        assert!(matches!(cp_err, ControlPlaneError::Storage(_)));
        assert!(cp_err.to_string().contains("file missing"));
    }

    // --- GatewayError -> HTTP status code mapping ---

    #[test]
    fn gateway_error_status_bad_request() {
        let resp = GatewayError::InvalidRequest("bad".into()).into_response();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    }

    #[test]
    fn gateway_error_status_conflict() {
        let resp = GatewayError::AgentAlreadyExists("a".into()).into_response();
        assert_eq!(resp.status(), StatusCode::CONFLICT);
    }

    #[test]
    fn gateway_error_status_not_found() {
        let resp = GatewayError::AgentNotFound("a".into()).into_response();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }

    #[test]
    fn gateway_error_status_too_many_requests() {
        let resp = GatewayError::RateLimitExceeded("limit".into()).into_response();
        assert_eq!(resp.status(), StatusCode::TOO_MANY_REQUESTS);
    }

    #[test]
    fn gateway_error_status_service_unavailable() {
        for err in [
            GatewayError::CircuitBreakerOpen("cb".into()),
            GatewayError::NoHealthyNodes("none".into()),
            GatewayError::UnhealthyNode("n".into()),
            GatewayError::NoAvailableNodes("none".into()),
            GatewayError::HealthCheckFailed("hc".into()),
        ] {
            let resp = err.into_response();
            assert_eq!(resp.status(), StatusCode::SERVICE_UNAVAILABLE);
        }
    }

    #[test]
    fn gateway_error_status_bad_gateway() {
        let resp = GatewayError::RoutingFailed("route".into()).into_response();
        assert_eq!(resp.status(), StatusCode::BAD_GATEWAY);

        let resp = GatewayError::Network("net".into()).into_response();
        assert_eq!(resp.status(), StatusCode::BAD_GATEWAY);
    }

    #[test]
    fn gateway_error_status_request_timeout() {
        let resp = GatewayError::Timeout("30s".into()).into_response();
        assert_eq!(resp.status(), StatusCode::REQUEST_TIMEOUT);
    }

    #[test]
    fn gateway_error_status_internal_server_error() {
        for err in [
            GatewayError::AgentOperationFailed("op".into()),
            GatewayError::LoadBalancing("lb".into()),
            GatewayError::Internal("err".into()),
        ] {
            let resp = err.into_response();
            assert_eq!(resp.status(), StatusCode::INTERNAL_SERVER_ERROR);
        }
    }
}
