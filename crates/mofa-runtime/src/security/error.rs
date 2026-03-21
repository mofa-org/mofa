//! Security Error Types
//!
//! Error types for security governance operations.

use thiserror::Error;

/// Security operation result
pub type SecurityResult<T> = Result<T, SecurityError>;

/// Security error types
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum SecurityError {
    /// Permission denied
    #[error("Permission denied: {0}")]
    PermissionDenied(String),

    /// PII detection error
    #[error("PII detection failed: {0}")]
    PiiDetectionFailed(String),

    /// PII redaction error
    #[error("PII redaction failed: {0}")]
    PiiRedactionFailed(String),

    /// Content moderation error
    #[error("Content moderation failed: {0}")]
    ContentModerationFailed(String),

    /// Prompt injection detected
    #[error("Prompt injection detected: {0}")]
    PromptInjectionDetected(String),

    /// Configuration error
    #[error("Security configuration error: {0}")]
    ConfigurationError(String),

    /// Internal error
    #[error("Internal security error: {0}")]
    Internal(String),
}
