//! Error types for the native dataflow module.

use thiserror::Error;

/// Errors that can occur in native dataflow operations.
#[derive(Error, Debug)]
#[non_exhaustive]
pub enum DataflowError {
    #[error("Node initialization failed: {0}")]
    NodeInitError(String),

    #[error("Node not running")]
    NodeNotRunning,

    #[error("Node not found: {0}")]
    NodeNotFound(String),

    #[error("Dataflow error: {0}")]
    DataflowError(String),

    #[error("Channel error: {0}")]
    ChannelError(String),

    #[error("Serialization error: {0}")]
    SerializationError(String),

    #[error("Deserialization error: {0}")]
    DeserializationError(String),

    #[error("Timeout: {0}")]
    Timeout(String),

    #[error("Agent not found: {0}")]
    AgentNotFound(String),

    #[error("Invalid configuration: {0}")]
    InvalidConfig(String),

    #[error("Runtime not running")]
    RuntimeNotRunning,

    #[error("Internal error: {0}")]
    Internal(String),
}

impl From<bincode::Error> for DataflowError {
    fn from(err: bincode::Error) -> Self {
        DataflowError::SerializationError(err.to_string())
    }
}

impl From<serde_json::Error> for DataflowError {
    fn from(err: serde_json::Error) -> Self {
        DataflowError::SerializationError(err.to_string())
    }
}

/// Result type for native dataflow operations.
pub type DataflowResult<T> = Result<T, DataflowError>;
