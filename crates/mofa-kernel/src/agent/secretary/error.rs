//! Typed errors for the secretary and connection sub-systems.

use thiserror::Error;

// Connection Errors
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
