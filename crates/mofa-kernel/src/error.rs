//! Crate-level error types for `mofa-kernel`.
//!
//! Provides a unified [`KernelError`] that composes errors from every
//! sub-module (agent, config, IO, serialization) together with
//! [`error_stack::Report`] for rich, context-carrying error propagation.
//!
//! # Usage
//!
//! ```rust,ignore
//! use mofa_kernel::error::{KernelError, KernelResult};
//! use error_stack::ResultExt;
//!
//! fn load_agent() -> KernelResult<()> {
//!     // Errors from sub-modules convert automatically via From impls.
//!     // Attach extra context with .change_context() / .attach().
//!     let config = std::fs::read_to_string("agent.toml")
//!         .map_err(KernelError::from)
//!         .map_err(error_stack::Report::new)
//!         .attach("loading agent.toml")?;
//!     Ok(())
//! }
//! ```

use crate::agent::error::AgentError;
use thiserror::Error;

/// Crate-level error type for `mofa-kernel`.
///
/// Wraps each sub-module's typed error via `#[from]` so that the `?`
/// operator converts them automatically. Use
/// [`error_stack::Report<KernelError>`] (via [`KernelResult`]) to attach
/// human-readable context as the error propagates up the call stack.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum KernelError {
    /// An error originating from the agent sub-system.
    #[error("Agent error: {0}")]
    Agent(#[from] AgentError),

    /// A configuration-related error (requires the `config` feature).
    #[cfg(feature = "config")]
    #[error("Config error: {0}")]
    Config(#[from] crate::config::ConfigError),

    /// A low-level I/O error.
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// A JSON (de)serialization error.
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    /// An internal / untyped error described by a message string.
    #[error("{0}")]
    Internal(String),

    /// A plugin sub-system error.
    #[error("Plugin error: {0}")]
    Plugin(#[from] crate::plugin::PluginError),

    /// A bus communication error.
    #[error("Bus error: {0}")]
    Bus(#[from] crate::bus::BusError),

    /// A connection-layer error.
    #[error("Connection error: {0}")]
    Connection(#[from] crate::agent::secretary::ConnectionError),

    /// A secretary-layer error.
    #[error("Secretary error: {0}")]
    Secretary(#[from] crate::agent::secretary::SecretaryError),
}

impl From<crate::agent::types::error::GlobalError> for KernelError {
    fn from(err: crate::agent::types::error::GlobalError) -> Self {
        use crate::agent::types::error::GlobalError;
        match err {
            GlobalError::Agent(e) => KernelError::Agent(e),
            GlobalError::Io(e) => KernelError::Io(e),
            GlobalError::Serialization(e) => KernelError::Serialization(e),
            GlobalError::LLM(msg)
            | GlobalError::Plugin(msg)
            | GlobalError::Runtime(msg)
            | GlobalError::Other(msg) => KernelError::Internal(msg),
        }
    }
}

/// Convenience result alias using [`error_stack::Report`].
///
/// Equivalent to `Result<T, error_stack::Report<KernelError>>`.
pub type KernelResult<T> = Result<T, error_stack::Report<KernelError>>;

// tests
#[cfg(test)]
mod tests {
    use super::*;
    use error_stack::{Report, ResultExt};

    #[test]
    fn agent_error_converts_via_from() {
        let agent_err = AgentError::NotFound("test-agent".to_string());
        let kernel_err: KernelError = agent_err.into();

        assert!(matches!(kernel_err, KernelError::Agent(_)));
        assert!(kernel_err.to_string().contains("test-agent"));
    }

    #[test]
    fn io_error_converts_via_from() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "file missing");
        let kernel_err: KernelError = io_err.into();

        assert!(matches!(kernel_err, KernelError::Io(_)));
        assert!(kernel_err.to_string().contains("file missing"));
    }

    #[test]
    fn serde_error_converts_via_from() {
        let bad_json = serde_json::from_str::<serde_json::Value>("not json");
        let serde_err = bad_json.unwrap_err();
        let kernel_err: KernelError = serde_err.into();

        assert!(matches!(kernel_err, KernelError::Serialization(_)));
    }

    #[test]
    fn internal_error_display() {
        let err = KernelError::Internal("something broke".into());
        assert_eq!(err.to_string(), "something broke");
    }

    #[test]
    fn report_carries_context() {
        let result: KernelResult<()> = Err(Report::new(KernelError::Internal("root cause".into())))
            .attach("while loading agent config");

        let report = result.unwrap_err();
        let display = format!("{report:?}");

        assert!(display.contains("root cause"));
        assert!(display.contains("while loading agent config"));
    }

    #[cfg(feature = "config")]
    #[test]
    fn config_error_converts_via_from() {
        let cfg_err = crate::config::ConfigError::UnsupportedFormat("xml".to_string());
        let kernel_err: KernelError = cfg_err.into();

        assert!(matches!(kernel_err, KernelError::Config(_)));
        assert!(kernel_err.to_string().contains("xml"));
    }

    #[test]
    fn global_error_converts_via_from() {
        use crate::agent::types::error::GlobalError;

        // AgentError variant
        let global_err = GlobalError::Agent(AgentError::NotFound("test".into()));
        let kernel_err: KernelError = global_err.into();
        assert!(matches!(kernel_err, KernelError::Agent(_)));

        // String variant
        let global_err = GlobalError::LLM("connection failed".into());
        let kernel_err: KernelError = global_err.into();
        assert!(matches!(kernel_err, KernelError::Internal(_)));
        assert!(kernel_err.to_string().contains("connection failed"));
    }
}
