//! Error types for adapter registry and resolution

use thiserror::Error;

/// Errors that can occur during adapter operations
#[derive(Debug, Error)]
pub enum AdapterError {
    #[error("Adapter with id '{0}' already registered")]
    AlreadyRegistered(String),

    #[error("Adapter with id '{0}' not found")]
    NotFound(String),

    #[error("Invalid adapter configuration: {0}")]
    InvalidConfig(String),

    #[error("Resolution failed: {0}")]
    ResolutionFailed(String),
}

/// Errors that can occur during adapter resolution
#[derive(Debug, Error)]
pub enum ResolutionError {
    #[error("No compatible adapter found for the given requirements")]
    NoCompatibleAdapter,

    #[error("Multiple compatible adapters found with equal score: {0:?}")]
    AmbiguousSelection(Vec<String>),

    #[error("Hardware requirements not met: {0}")]
    HardwareNotSupported(String),

    #[error("Format not supported: {0}")]
    FormatNotSupported(String),

    #[error("Modality not supported: {0}")]
    ModalityNotSupported(String),
}

/// Reasons why an adapter was rejected during resolution
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RejectionReason {
    /// Adapter does not support the required modality
    ModalityMismatch {
        required: String,
        supported: Vec<String>,
    },
    /// Adapter does not support the required model format
    FormatMismatch {
        required: String,
        supported: Vec<String>,
    },
    /// Required quantization not available
    QuantizationMismatch {
        required: String,
        supported: Vec<String>,
    },
    /// Hardware constraints not met
    HardwareConstraint {
        constraint: String,
        reason: String,
    },
    /// Memory requirements not met
    MemoryInsufficient {
        required_mb: u64,
        available_mb: u64,
    },
    /// Priority is too low
    PriorityTooLow {
        required_min_priority: i32,
        adapter_priority: i32,
    },
}

impl std::fmt::Display for RejectionReason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RejectionReason::ModalityMismatch { required, supported } => {
                write!(
                    f,
                    "Modality mismatch: required '{}', supported {:?}",
                    required, supported
                )
            }
            RejectionReason::FormatMismatch { required, supported } => {
                write!(
                    f,
                    "Format mismatch: required '{}', supported {:?}",
                    required, supported
                )
            }
            RejectionReason::QuantizationMismatch { required, supported } => {
                write!(
                    f,
                    "Quantization mismatch: required '{}', supported {:?}",
                    required, supported
                )
            }
            RejectionReason::HardwareConstraint { constraint, reason } => {
                write!(
                    f,
                    "Hardware constraint not met: {} - {}",
                    constraint, reason
                )
            }
            RejectionReason::MemoryInsufficient {
                required_mb,
                available_mb,
            } => {
                write!(
                    f,
                    "Insufficient memory: required {}MB, available {}MB",
                    required_mb, available_mb
                )
            }
            RejectionReason::PriorityTooLow {
                required_min_priority,
                adapter_priority,
            } => {
                write!(
                    f,
                    "Priority too low: required min {}, got {}",
                    required_min_priority, adapter_priority
                )
            }
        }
    }
}

impl RejectionReason {
    /// Returns the severity level of this rejection reason
    pub fn severity(&self) -> RejectionSeverity {
        match self {
            RejectionReason::ModalityMismatch { .. } => RejectionSeverity::Hard,
            RejectionReason::FormatMismatch { .. } => RejectionSeverity::Hard,
            RejectionReason::QuantizationMismatch { .. } => RejectionSeverity::Hard,
            RejectionReason::HardwareConstraint { .. } => RejectionSeverity::Hard,
            RejectionReason::MemoryInsufficient { .. } => RejectionSeverity::Hard,
            RejectionReason::PriorityTooLow { .. } => RejectionSeverity::Soft,
        }
    }
}

/// Severity level of a rejection reason
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum RejectionSeverity {
    /// Hard constraint - adapter cannot be selected
    Hard,
    /// Soft constraint - adapter can still be selected but with lower priority
    Soft,
}
