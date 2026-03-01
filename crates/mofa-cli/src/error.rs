//! Typed error types and result aliases for the MoFA CLI.
//!
//! # Usage
//!
//! ```rust,ignore
//! use crate::error::{CliResult, IntoCliReport as _};
//! use error_stack::ResultExt as _;
//!
//! fn parse_config(path: &str) -> CliResult<()> {
//!     std::fs::read_to_string(path)
//!         .map_err(CliError::Io)
//!         .into_report()
//!         .attach(format!("reading {path}"))?;
//!     Ok(())
//! }
//! ```

use error_stack::Report;
use std::path::PathBuf;

/// CLI error type.
///
/// Every variant maps to one failure domain so that error-stack context
/// injected through `IntoCliReport::into_report()` stays structured.
#[derive(thiserror::Error, Debug)]
#[non_exhaustive]
pub enum CliError {
    #[error("Configuration error: {0}")]
    ConfigError(String),

    #[error("Session error: {0}")]
    SessionError(String),

    #[error("Plugin error: {0}")]
    PluginError(String),

    #[error("Tool error: {0}")]
    ToolError(String),

    #[error("State error: {0}")]
    StateError(String),

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Dialoguer error: {0}")]
    DialoguerError(#[from] dialoguer::Error),

    #[error("Serialization error: {0}")]
    SerializationError(#[from] serde_json::Error),

    #[error("YAML error: {0}")]
    YamlError(#[from] serde_yaml::Error),

    #[error("API error: {0}")]
    ApiError(String),

    #[error("Parse integer error: {0}")]
    ParseInt(#[from] std::num::ParseIntError),

    #[error("Parse float error: {0}")]
    ParseFloat(#[from] std::num::ParseFloatError),

    #[error("Mofa SDK error: {0}")]
    Sdk(String),

    #[error("Initialization error: {0}")]
    InitError(String),

    #[error("{0}")]
    Other(String),
}

/// Canonical result type for CLI operations.
///
/// Equivalent to `Result<T, error_stack::Report<CliError>>`.  Carrying the
/// full `Report` means every intermediate context annotation (attached via
/// `.attach()` or `.change_context()`) is preserved and printed in the
/// debug-formatted output when an error reaches `main()`.
pub type CliResult<T> = ::std::result::Result<T, Report<CliError>>;

/// Extension trait to convert `Result<T, CliError>` into [`CliResult<T>`].
///
/// Call `.into_report()` at the point where a plain `CliError` value is first
/// produced, then chain `.attach("context")` before propagating with `?`.
pub trait IntoCliReport<T> {
    /// Wrap the bare `CliError` in an `error_stack::Report`.
    fn into_report(self) -> CliResult<T>;
}

impl<T> IntoCliReport<T> for Result<T, CliError> {
    #[inline]
    fn into_report(self) -> CliResult<T> {
        self.map_err(Report::new)
    }
}

/// Install the global `error_stack` display hook.
///
/// In **release** builds this suppresses file/line stack-frame annotations to
/// produce cleaner user-facing output. In **debug** builds all frames are kept
/// so the full propagation chain is visible during development.
///
/// Call this once at the very start of `main()`, before any fallible work.
pub fn install_hook() {
    #[cfg(not(debug_assertions))]
    Report::install_debug_hook::<std::panic::Location>(|_, _| {});
}

// ── Plain string conversions (kept for call-site ergonomics) ──────────────

impl From<&str> for CliError {
    fn from(s: &str) -> Self {
        CliError::Other(s.to_string())
    }
}

impl From<String> for CliError {
    fn from(s: String) -> Self {
        CliError::Other(s)
    }
}
