//! Sandbox errors
//!
//! Error types for tool sandbox construction, policy evaluation, and
//! sandboxed execution.

use std::time::Duration;
use thiserror::Error;

/// Errors raised by a tool sandbox layer.
///
/// `SandboxError` is kept distinct from [`crate::agent::error::AgentError`] so
/// a sandboxed tool failure can be distinguished from a generic tool error —
/// a policy denial is not the same class of event as a tool bug.
///
/// Downstream conversion into `AgentError` is provided via `From`.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum SandboxError {
    /// The requested capability is not present in the active policy.
    #[error("capability denied: tool `{tool}` requested `{capability}` but policy only permits {allowed:?}")]
    CapabilityDenied {
        tool: String,
        capability: String,
        allowed: Vec<String>,
    },

    /// A filesystem path was accessed that falls outside the policy allow-list.
    #[error("filesystem path `{path}` is outside the allow-list for tool `{tool}`")]
    PathNotAllowed { tool: String, path: String },

    /// A network destination was not permitted.
    #[error("network destination `{host}:{port}` not permitted for tool `{tool}`")]
    NetworkNotAllowed {
        tool: String,
        host: String,
        port: u16,
    },

    /// An environment variable read/write was not permitted.
    #[error("environment variable `{name}` not readable under current policy")]
    EnvVarNotAllowed { name: String },

    /// Subprocess spawning is disabled or the binary is not in the allow-list.
    #[error("subprocess `{program}` not permitted for tool `{tool}`")]
    SubprocessNotAllowed { tool: String, program: String },

    /// CPU-time resource limit exhausted.
    #[error("tool `{tool}` exceeded CPU time limit of {limit:?} (observed {observed:?})")]
    CpuTimeExceeded {
        tool: String,
        limit: Duration,
        observed: Duration,
    },

    /// Wall-clock timeout.
    #[error("tool `{tool}` exceeded wall-clock timeout of {limit:?}")]
    WallTimeout { tool: String, limit: Duration },

    /// Memory usage exceeded the configured cap.
    #[error("tool `{tool}` exceeded memory cap of {limit_bytes} bytes (observed {observed_bytes})")]
    MemoryExceeded {
        tool: String,
        limit_bytes: u64,
        observed_bytes: u64,
    },

    /// Output size from the sandbox exceeded the configured cap.
    #[error(
        "tool `{tool}` produced output of {observed_bytes} bytes, exceeding cap of {limit_bytes}"
    )]
    OutputTooLarge {
        tool: String,
        limit_bytes: u64,
        observed_bytes: u64,
    },

    /// The sandboxed process terminated abnormally (signal, OOM-kill, etc).
    #[error("tool `{tool}` crashed inside sandbox: {reason}")]
    SandboxCrashed { tool: String, reason: String },

    /// The sandbox backend itself failed to set up (fork failure, wasm engine
    /// error, rlimit syscall error, etc).
    #[error("sandbox backend error: {0}")]
    BackendFailure(String),

    /// Policy was malformed or internally inconsistent (e.g. conflicting
    /// capability lists). Raised at construction time, not at execution.
    #[error("invalid sandbox policy: {0}")]
    InvalidPolicy(String),

    /// Serialization of sandboxed I/O failed.
    #[error("sandbox I/O serialization error: {0}")]
    SerializationError(String),

    /// Backend-specific I/O error that doesn't fit the categories above.
    #[error("sandbox I/O error: {0}")]
    IoError(String),
}

impl SandboxError {
    /// Returns `true` if the error represents a policy denial (the tool
    /// attempted something the policy forbids) as opposed to a backend
    /// failure or resource-limit breach.
    ///
    /// Policy denials are generally *deterministic* — the same tool call
    /// with the same policy will always be denied — so callers can use this
    /// to decide whether a retry makes sense.
    pub fn is_policy_denial(&self) -> bool {
        matches!(
            self,
            SandboxError::CapabilityDenied { .. }
                | SandboxError::PathNotAllowed { .. }
                | SandboxError::NetworkNotAllowed { .. }
                | SandboxError::EnvVarNotAllowed { .. }
                | SandboxError::SubprocessNotAllowed { .. }
        )
    }

    /// Returns `true` if the error is a resource-limit breach (CPU, memory,
    /// wall time, output size). These are not retryable without adjusting
    /// the policy or the tool's workload.
    pub fn is_resource_limit(&self) -> bool {
        matches!(
            self,
            SandboxError::CpuTimeExceeded { .. }
                | SandboxError::WallTimeout { .. }
                | SandboxError::MemoryExceeded { .. }
                | SandboxError::OutputTooLarge { .. }
        )
    }

    /// Returns `true` if the error indicates a backend-internal failure
    /// rather than a user-visible policy outcome. Backend failures may be
    /// retryable (e.g. transient fork EAGAIN).
    pub fn is_backend_failure(&self) -> bool {
        matches!(
            self,
            SandboxError::BackendFailure(_)
                | SandboxError::IoError(_)
                | SandboxError::SandboxCrashed { .. }
        )
    }
}

impl From<std::io::Error> for SandboxError {
    fn from(e: std::io::Error) -> Self {
        SandboxError::IoError(e.to_string())
    }
}

impl From<serde_json::Error> for SandboxError {
    fn from(e: serde_json::Error) -> Self {
        SandboxError::SerializationError(e.to_string())
    }
}

impl From<SandboxError> for crate::agent::error::AgentError {
    fn from(e: SandboxError) -> Self {
        use crate::agent::error::AgentError;
        match e {
            SandboxError::CapabilityDenied { .. }
            | SandboxError::PathNotAllowed { .. }
            | SandboxError::NetworkNotAllowed { .. }
            | SandboxError::EnvVarNotAllowed { .. }
            | SandboxError::SubprocessNotAllowed { .. } => AgentError::ValidationFailed(e.to_string()),

            SandboxError::CpuTimeExceeded { limit, .. } | SandboxError::WallTimeout { limit, .. } => {
                AgentError::Timeout {
                    duration_ms: limit.as_millis() as u64,
                }
            }

            SandboxError::MemoryExceeded { .. } | SandboxError::OutputTooLarge { .. } => {
                AgentError::ResourceUnavailable(e.to_string())
            }

            SandboxError::SandboxCrashed { .. } | SandboxError::BackendFailure(_) => {
                AgentError::ExecutionFailed(e.to_string())
            }

            SandboxError::InvalidPolicy(_) => AgentError::ConfigError(e.to_string()),
            SandboxError::SerializationError(_) => AgentError::SerializationError(e.to_string()),
            SandboxError::IoError(_) => AgentError::IoError(e.to_string()),
        }
    }
}

/// Convenient result alias for sandbox operations.
pub type SandboxResult<T> = Result<T, SandboxError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn policy_denial_classification() {
        let e = SandboxError::CapabilityDenied {
            tool: "curl".into(),
            capability: "Net".into(),
            allowed: vec!["Fs".into()],
        };
        assert!(e.is_policy_denial());
        assert!(!e.is_resource_limit());
        assert!(!e.is_backend_failure());
    }

    #[test]
    fn resource_limit_classification() {
        let e = SandboxError::WallTimeout {
            tool: "slow".into(),
            limit: Duration::from_secs(1),
        };
        assert!(e.is_resource_limit());
        assert!(!e.is_policy_denial());
        assert!(!e.is_backend_failure());
    }

    #[test]
    fn backend_failure_classification() {
        let e = SandboxError::BackendFailure("fork failed".into());
        assert!(e.is_backend_failure());
        assert!(!e.is_policy_denial());
        assert!(!e.is_resource_limit());
    }

    #[test]
    fn converts_to_agent_error_as_validation() {
        let e = SandboxError::PathNotAllowed {
            tool: "cat".into(),
            path: "/etc/passwd".into(),
        };
        let ae: crate::agent::error::AgentError = e.into();
        assert!(matches!(
            ae,
            crate::agent::error::AgentError::ValidationFailed(_)
        ));
    }

    #[test]
    fn converts_timeout_preserving_duration() {
        let e = SandboxError::WallTimeout {
            tool: "slow".into(),
            limit: Duration::from_millis(500),
        };
        let ae: crate::agent::error::AgentError = e.into();
        match ae {
            crate::agent::error::AgentError::Timeout { duration_ms } => assert_eq!(duration_ms, 500),
            other => panic!("unexpected variant: {other:?}"),
        }
    }
}
