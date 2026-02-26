//! Typed errors for the Rhai scripting subsystem.
//!
//! Rhai engine, tool, workflow, and rule operations.

use thiserror::Error;

/// Errors that can occur in the Rhai scripting subsystem.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum RhaiError {
    /// Script compilation failed.
    #[error("Compile error: {0}")]
    CompileError(String),

    /// Script execution failed at runtime.
    #[error("Execution error: {0}")]
    ExecutionError(String),

    /// An I/O error (e.g. reading a script file).
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// A (de)serialization error.
    #[error("Serialization error: {0}")]
    Serialization(String),

    /// Validation of input/parameters failed.
    #[error("Validation error: {0}")]
    ValidationError(String),

    /// A requested resource was not found.
    #[error("Not found: {0}")]
    NotFound(String),

    /// Catch-all.
    #[error("{0}")]
    Other(String),
}

/// Convenience result alias for the Rhai subsystem.
pub type RhaiResult<T> = Result<T, RhaiError>;

// Conversion helpers
impl From<serde_json::Error> for RhaiError {
    fn from(err: serde_json::Error) -> Self {
        RhaiError::Serialization(err.to_string())
    }
}

impl From<serde_yaml::Error> for RhaiError {
    fn from(err: serde_yaml::Error) -> Self {
        RhaiError::Serialization(err.to_string())
    }
}
