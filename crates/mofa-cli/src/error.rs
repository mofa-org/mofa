//! Error types and `error-stack` integration for the MoFA CLI.
//!
//! # Design
//!
//! [`CliError`] is the single canonical error context.  All commands return
//! [`CliResult<T>`], which is an alias for `error_stack::Result<T, CliError>`.
//!
//! ## Migrating call-sites
//!
//! Old style (thiserror only):
//! ```rust,ignore
//! fn foo() -> Result<(), CliError> { ... }
//! ```
//!
//! New style (error-stack):
//! ```rust,ignore
//! use crate::error::{CliResult, IntoCliReport as _};
//! use error_stack::ResultExt as _;
//!
//! fn foo() -> CliResult<()> {
//!     bar()
//!         .into_report()
//!         .attach("while doing foo")
//! }
//! ```
//!
//! Because [`From<CliError>`] is implemented for [`error_stack::Report<CliError>`],
//! the plain `?` operator continues to work unchanged at any call-site that already
//! returns a `CliResult`.

use error_stack::Report;

// ── Core error enum ──────────────────────────────────────────────────────────

/// Unified error context for every MoFA CLI command.
///
/// Implements [`std::error::Error`] via [`thiserror`], which satisfies the
/// `error_stack::Context` bound (`Display + Debug + Send + Sync + 'static`).
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum CliError {
    /// A problem loading, parsing, or validating agent or CLI configuration.
    #[error("Configuration error: {0}")]
    ConfigError(String),

    /// A problem with session persistence or retrieval.
    #[error("Session error: {0}")]
    SessionError(String),

    /// A problem installing, uninstalling, or querying a plugin.
    #[error("Plugin error: {0}")]
    PluginError(String),

    /// A problem registering or invoking a tool.
    #[error("Tool error: {0}")]
    ToolError(String),

    /// A problem reading or writing persisted agent state.
    #[error("State error: {0}")]
    StateError(String),

    /// An operating-system I/O failure.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// An interactive-prompt (dialoguer) failure.
    #[error("Prompt error: {0}")]
    DialoguerError(#[from] dialoguer::Error),

    /// A JSON serialization / deserialization failure.
    #[error("JSON error: {0}")]
    SerializationError(#[from] serde_json::Error),

    /// A YAML serialization / deserialization failure.
    #[error("YAML error: {0}")]
    YamlError(#[from] serde_yaml::Error),

    /// An upstream API call failure.
    #[error("API error: {0}")]
    ApiError(String),

    /// An integer parse failure.
    #[error("Integer parse error: {0}")]
    ParseInt(#[from] std::num::ParseIntError),

    /// A float parse failure.
    #[error("Float parse error: {0}")]
    ParseFloat(#[from] std::num::ParseFloatError),

    /// An error propagated from the MoFA SDK.
    #[error("Mofa SDK error: {0}")]
    Sdk(String),

    /// A failure during application or context initialization.
    #[error("Initialization error: {0}")]
    InitError(String),

    /// A catch-all for errors that don't fit another variant.
    #[error("{0}")]
    Other(String),
}

// ── CliResult ────────────────────────────────────────────────────────────────

/// The canonical result type for every MoFA CLI command.
///
/// `CliResult<T>` is `std::result::Result<T, error_stack::Report<CliError>>`,
///
/// Use [`error_stack::ResultExt`] on a `CliResult` to attach additional
/// human-readable context:
///
/// ```rust,ignore
/// use error_stack::ResultExt as _;
///
/// read_file(path)
///     .into_report()
///     .attach_with(|| format!("path: {}", path.display()))
/// ```
pub type CliResult<T> = ::std::result::Result<T, error_stack::Report<CliError>>;

// ── From<CliError> for Report ─────────────────────────────────────────────────
//
// NOTE: error-stack 0.6 provides a blanket `impl<C: Context> From<C> for Report<C>`
// automatically.  The plain `?` operator therefore already works at any call-site
// whose return type is `CliResult<T>` — no manual impl required.

// ── IntoCliReport ─────────────────────────────────────────────────────────────

/// Extension trait to convert a `Result<T, CliError>` into a [`CliResult<T>`].
///
/// Use this at module boundaries where a function still returns the plain
/// `Result<T, CliError>` type and you want to enter the `error_stack` world:
///
/// ```rust,ignore
/// use crate::error::IntoCliReport as _;
/// use error_stack::ResultExt as _;
///
/// commands::plugin::install::run(ctx, name)
///     .await
///     .into_report()
///     .attach("installing plugin")
/// ```
pub trait IntoCliReport<T> {
    /// Wrap the error in an `error_stack::Report`, capturing the current
    /// location as the first stack frame.
    fn into_report(self) -> CliResult<T>;
}

impl<T> IntoCliReport<T> for std::result::Result<T, CliError> {
    #[inline]
    fn into_report(self) -> CliResult<T> {
        self.map_err(Report::new)
    }
}

// ── Hook installation ─────────────────────────────────────────────────────────

/// Install the global `error_stack` debug hooks for production-quality CLI output.
///
/// Call **once** at the very beginning of `main()`, before any errors can be
/// produced.  Hooks are global and cannot be uninstalled; calling this function
/// multiple times is a no-op after the first call.
///
/// ## Output example
///
/// ```text
/// mofa: error: Plugin error: failed to verify checksum
///
///    ├─ downloading plugin 'llm-openai' from repository
///    ├─ plugin install
///    └─ backtrace omitted — set RUST_BACKTRACE=1 to enable
/// ```
pub fn install_hook() {
    // Suppress source-location frames in release builds; show them only when
    // RUST_BACKTRACE is set or debug assertions are active.
    Report::install_debug_hook::<std::panic::Location>(|location, ctx| {
        if std::env::var("RUST_BACKTRACE").is_ok() || cfg!(debug_assertions) {
            ctx.push_body(format!(
                "at {}:{}:{}",
                location.file(),
                location.line(),
                location.column(),
            ));
        }
    });
}

// ── String convenience impls ──────────────────────────────────────────────────

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
