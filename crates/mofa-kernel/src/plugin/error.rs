//! Typed errors for the plugin sub-system.

use thiserror::Error;

/// Errors that can occur during plugin lifecycle operations.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum PluginError {
    /// Plugin failed during the `load` phase.
    #[error("Plugin load failed: {0}")]
    LoadFailed(String),

    /// Plugin failed during initialisation.
    #[error("Plugin initialization failed: {0}")]
    InitFailed(String),

    /// Plugin execution (`execute`) returned an error.
    #[error("Plugin execution failed: {0}")]
    ExecutionFailed(String),

    /// An operation was attempted while the plugin was in an incompatible state.
    #[error("Plugin not in valid state: expected {expected}, got {actual}")]
    InvalidState {
        /// The state(s) that were expected.
        expected: String,
        /// The state the plugin was actually in.
        actual: String,
    },

    /// Plugin configuration is invalid or missing.
    #[error("Plugin configuration error: {0}")]
    ConfigError(String),

    /// An I/O error surfaced during a plugin operation.
    #[error("Plugin I/O error: {source}")]
    Io {
        #[from]
        source: std::io::Error,
    },

    /// A (de)serialization error surfaced during a plugin operation.
    #[error("Plugin serialization error: {source}")]
    Serialization {
        #[from]
        source: serde_json::Error,
    },

    /// Catch-all for errors that don't fit the above categories.
    #[error("{0}")]
    Other(String),
}
