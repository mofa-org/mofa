//! Admission decision types for the memory scheduler.

use serde::{Deserialize, Serialize};

/// The outcome of an admission evaluation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AdmissionOutcome {
    /// Request is accepted — memory is available.
    Accept,
    /// Request is deferred — memory is tight but may free up.
    Defer,
    /// Request is rejected — exceeds capacity.
    Reject,
}

impl AdmissionOutcome {
    /// Whether the request can be retried later.
    pub fn is_retryable(&self) -> bool {
        matches!(self, Self::Defer)
    }

    /// Whether this is a terminal decision.
    pub fn is_final(&self) -> bool {
        matches!(self, Self::Accept | Self::Reject)
    }
}

impl std::fmt::Display for AdmissionOutcome {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Accept => write!(f, "Accept"),
            Self::Defer => write!(f, "Defer"),
            Self::Reject => write!(f, "Reject"),
        }
    }
}

/// Full admission decision with diagnostic metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdmissionDecision {
    /// The outcome.
    pub outcome: AdmissionOutcome,
    /// Human-readable reason for the decision.
    pub reason: String,
    /// Current memory usage at decision time (MB).
    pub current_usage_mb: u64,
    /// Memory required by the request (MB).
    pub required_mb: u64,
    /// Memory available at decision time (MB).
    pub available_mb: u64,
}

impl AdmissionDecision {
    /// Convenience: is this an Accept?
    pub fn is_accepted(&self) -> bool {
        self.outcome == AdmissionOutcome::Accept
    }

    /// Convenience: is this a Defer?
    pub fn is_deferred(&self) -> bool {
        self.outcome == AdmissionOutcome::Defer
    }

    /// Convenience: is this a Reject?
    pub fn is_rejected(&self) -> bool {
        self.outcome == AdmissionOutcome::Reject
    }
}
