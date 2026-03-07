//! Typed errors and result aliases for the secretary and connection sub-systems.

use error_stack::Report;
use thiserror::Error;

// ── Connection layer ──────────────────────────────────────────────────────────

/// Error-stack–backed result alias for connection operations.
///
/// Equivalent to `Result<T, error_stack::Report<ConnectionError>>`.
pub type ConnectionResult<T> = ::std::result::Result<T, Report<ConnectionError>>;

/// Extension trait to convert `Result<T, ConnectionError>` into
/// [`ConnectionResult<T>`].
pub trait IntoConnectionReport<T> {
    /// Wrap the error in an `error_stack::Report`.
    fn into_report(self) -> ConnectionResult<T>;
}

impl<T> IntoConnectionReport<T> for ::std::result::Result<T, ConnectionError> {
    #[inline]
    fn into_report(self) -> ConnectionResult<T> {
        self.map_err(Report::new)
    }
}

// ── Secretary layer ───────────────────────────────────────────────────────────

/// Error-stack–backed result alias for secretary operations.
///
/// Equivalent to `Result<T, error_stack::Report<SecretaryError>>`.
pub type SecretaryResult<T> = ::std::result::Result<T, Report<SecretaryError>>;

/// Extension trait to convert `Result<T, SecretaryError>` into
/// [`SecretaryResult<T>`].
pub trait IntoSecretaryReport<T> {
    /// Wrap the error in an `error_stack::Report`.
    fn into_report(self) -> SecretaryResult<T>;
}

impl<T> IntoSecretaryReport<T> for ::std::result::Result<T, SecretaryError> {
    #[inline]
    fn into_report(self) -> SecretaryResult<T> {
        self.map_err(Report::new)
    }
}

/// Errors that can occur on the user-connection layer.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum ConnectionError {
    /// The connection has been closed (gracefully or otherwise).
    #[error("Connection closed")]
    Closed,

    /// A connection-level timeout expired.
    #[error("Connection timeout")]
    Timeout,

    /// Sending a message to the remote peer failed.
    #[error("Send failed: {0}")]
    SendFailed(String),

    /// Receiving a message from the remote peer failed.
    #[error("Receive failed: {0}")]
    ReceiveFailed(String),

    /// An underlying I/O error.
    #[error("I/O error: {source}")]
    Io {
        #[from]
        source: std::io::Error,
    },

    /// A (de)serialization error at the connection boundary.
    #[error("Serialization error: {0}")]
    Serialization(String),

    /// Catch-all for errors that don't fit the above categories.
    #[error("{0}")]
    Other(String),
}

// Secretary Errors
/// Errors that can occur in the secretary behaviour layer.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum SecretaryError {
    /// An error occurred while handling user input.
    #[error("Input handling failed: {0}")]
    InputHandlingFailed(String),

    /// An error propagated from the connection layer.
    #[error("Connection error: {source}")]
    Connection {
        #[from]
        source: ConnectionError,
    },

    /// A workflow orchestration step failed.
    #[error("Workflow error: {0}")]
    WorkflowFailed(String),

    /// A single phase within a workflow failed.
    #[error("Phase error: {0}")]
    PhaseFailed(String),

    /// Catch-all for errors that don't fit the above categories.
    #[error("{0}")]
    Other(String),
}
