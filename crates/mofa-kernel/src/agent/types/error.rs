//! Global Error Type System
//!
//! This module provides a global error hierarchy, integrating error types from all layers.
//!
//! # Design Goals
//!
//! - Provide global error abstraction, avoiding multiple error types (AgentResult, LLMResult, PluginResult)
//! - Preserve specific information from errors in each layer
//! - Support error chaining and context
//! - Provide clear identification of error sources
//! - Enable cross-crate error conversion via `From` impls
//! - Support error severity classification and recovery strategies

use crate::agent::error::AgentError;
use std::fmt;

// ============================================================================
// GlobalError - 全局错误类型
// GlobalError - Global Error Type
// ============================================================================

/// Global error type
///
/// Integrates error types from all layers, providing a single error abstraction.
/// Downstream crates implement `From<TheirError> for GlobalError` to enable
/// seamless `?` operator usage across crate boundaries.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum GlobalError {
    // ---- Core layer errors ----

    /// Agent layer error
    #[error("Agent error: {0}")]
    Agent(#[from] AgentError),

    /// LLM layer error
    #[error("LLM error: {0}")]
    LLM(String),

    /// Plugin error (WASM, Rhai, or other plugin systems)
    #[error("Plugin error: {0}")]
    Plugin(String),

    /// Runtime error
    #[error("Runtime error: {0}")]
    Runtime(String),

    // ---- Infrastructure errors ----

    /// IO error
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// Serialization / deserialization error
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    /// Configuration error
    #[error("Config error: {0}")]
    Config(String),

    // ---- Domain-specific errors ----

    /// Persistence / storage error
    #[error("Persistence error: {0}")]
    Persistence(String),

    /// Prompt template error
    #[error("Prompt error: {0}")]
    Prompt(String),

    /// Workflow DSL error
    #[error("DSL error: {0}")]
    Dsl(String),

    /// WASM runtime error
    #[error("WASM error: {0}")]
    Wasm(String),

    /// Rhai scripting error
    #[error("Rhai error: {0}")]
    Rhai(String),

    /// Dora adapter error
    #[error("Dora error: {0}")]
    Dora(String),

    /// Message graph error
    #[error("MessageGraph error: {0}")]
    MessageGraph(String),

    // ---- Catch-all ----

    /// Other / untyped error
    #[error("Other error: {0}")]
    Other(String),
}

impl GlobalError {
    // ---- Constructors ----

    /// Create LLM error
    pub fn llm(msg: impl Into<String>) -> Self {
        Self::LLM(msg.into())
    }

    /// Create plugin error
    pub fn plugin(msg: impl Into<String>) -> Self {
        Self::Plugin(msg.into())
    }

    /// Create runtime error
    pub fn runtime(msg: impl Into<String>) -> Self {
        Self::Runtime(msg.into())
    }

    /// Create config error
    pub fn config(msg: impl Into<String>) -> Self {
        Self::Config(msg.into())
    }

    /// Create persistence error
    pub fn persistence(msg: impl Into<String>) -> Self {
        Self::Persistence(msg.into())
    }

    /// Create prompt error
    pub fn prompt(msg: impl Into<String>) -> Self {
        Self::Prompt(msg.into())
    }

    /// Create DSL error
    pub fn dsl(msg: impl Into<String>) -> Self {
        Self::Dsl(msg.into())
    }

    /// Create WASM error
    pub fn wasm(msg: impl Into<String>) -> Self {
        Self::Wasm(msg.into())
    }

    /// Create Rhai error
    pub fn rhai(msg: impl Into<String>) -> Self {
        Self::Rhai(msg.into())
    }

    /// Create Dora error
    pub fn dora(msg: impl Into<String>) -> Self {
        Self::Dora(msg.into())
    }

    /// Create message graph error
    pub fn message_graph(msg: impl Into<String>) -> Self {
        Self::MessageGraph(msg.into())
    }

    // ---- Classification ----

    /// Get error category
    pub fn category(&self) -> ErrorCategory {
        match self {
            Self::Agent(_) => ErrorCategory::Agent,
            Self::LLM(_) => ErrorCategory::LLM,
            Self::Plugin(_) | Self::Wasm(_) | Self::Rhai(_) => ErrorCategory::Plugin,
            Self::Runtime(_) | Self::Dora(_) => ErrorCategory::Runtime,
            Self::Io(_) => ErrorCategory::Io,
            Self::Serialization(_) => ErrorCategory::Serialization,
            Self::Config(_) => ErrorCategory::Config,
            Self::Persistence(_) => ErrorCategory::Persistence,
            Self::Prompt(_) | Self::Dsl(_) => ErrorCategory::Workflow,
            Self::MessageGraph(_) => ErrorCategory::MessageGraph,
            Self::Other(_) => ErrorCategory::Other,
        }
    }

    /// Get error severity
    pub fn severity(&self) -> ErrorSeverity {
        match self {
            Self::Agent(AgentError::InvalidStateTransition { .. }) => ErrorSeverity::Fatal,
            Self::Agent(AgentError::ConfigError(_)) | Self::Config(_) => ErrorSeverity::Error,
            Self::Runtime(_) | Self::Io(_) | Self::LLM(_) => ErrorSeverity::Retryable,
            Self::Plugin(_) | Self::Wasm(_) | Self::Rhai(_) => ErrorSeverity::Retryable,
            Self::Dora(_) => ErrorSeverity::Retryable,
            _ => ErrorSeverity::Error,
        }
    }

    /// Whether it is a retryable error
    pub fn is_retryable(&self) -> bool {
        matches!(self.severity(), ErrorSeverity::Retryable)
    }

    /// Whether it is a fatal error
    pub fn is_fatal(&self) -> bool {
        matches!(self.severity(), ErrorSeverity::Fatal)
    }

    /// Attach additional context to this error
    pub fn with_context(self, ctx: ErrorContext) -> ContextualError {
        ContextualError {
            error: self,
            context: ctx,
        }
    }
}

// ============================================================================
// ErrorSeverity - Error Severity Level
// ============================================================================

/// Error severity level
///
/// Classifies errors by their impact and whether recovery is possible.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ErrorSeverity {
    /// Fatal: unrecoverable, the operation cannot proceed
    Fatal,
    /// Error: the operation failed, but the system is still functional
    Error,
    /// Retryable: the operation failed, but retrying may succeed
    Retryable,
    /// Warning: the operation completed, but with issues
    Warning,
}

impl fmt::Display for ErrorSeverity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Fatal => write!(f, "fatal"),
            Self::Error => write!(f, "error"),
            Self::Retryable => write!(f, "retryable"),
            Self::Warning => write!(f, "warning"),
        }
    }
}

// ============================================================================
// ErrorCategory - Error Category
// ============================================================================

/// Error category
///
/// Used for error statistics, monitoring, and routing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum ErrorCategory {
    /// Agent related errors
    Agent,
    /// LLM related errors
    LLM,
    /// Plugin related errors (WASM, Rhai, etc.)
    Plugin,
    /// Runtime errors (Dora, message bus, etc.)
    Runtime,
    /// IO errors
    Io,
    /// Serialization errors
    Serialization,
    /// Configuration errors
    Config,
    /// Persistence / storage errors
    Persistence,
    /// Workflow / DSL / Prompt errors
    Workflow,
    /// Message graph errors
    MessageGraph,
    /// Other errors
    Other,
}

impl fmt::Display for ErrorCategory {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Agent => write!(f, "agent"),
            Self::LLM => write!(f, "llm"),
            Self::Plugin => write!(f, "plugin"),
            Self::Runtime => write!(f, "runtime"),
            Self::Io => write!(f, "io"),
            Self::Serialization => write!(f, "serialization"),
            Self::Config => write!(f, "config"),
            Self::Persistence => write!(f, "persistence"),
            Self::Workflow => write!(f, "workflow"),
            Self::MessageGraph => write!(f, "message_graph"),
            Self::Other => write!(f, "other"),
        }
    }
}

// ============================================================================
// GlobalResult - Global Result Type
// ============================================================================

/// Global result type
///
/// Can be used at crate boundaries to replace `AgentResult`, `LLMResult`,
/// `PluginResult`, etc., providing a single result type.
pub type GlobalResult<T> = Result<T, GlobalError>;

// ============================================================================
// Conversion from other error types
// ============================================================================

impl From<anyhow::Error> for GlobalError {
    fn from(err: anyhow::Error) -> Self {
        Self::Other(err.to_string())
    }
}

impl From<String> for GlobalError {
    fn from(s: String) -> Self {
        Self::Other(s)
    }
}

impl From<&str> for GlobalError {
    fn from(s: &str) -> Self {
        Self::Other(s.to_string())
    }
}

// ============================================================================
// ErrorContext - Error Context
// ============================================================================

/// Error context
///
/// Carries additional error information for debugging and diagnostics.
#[derive(Debug, Clone)]
pub struct ErrorContext {
    /// Error message
    pub message: String,
    /// Error location (e.g. "agent.rs:42")
    pub location: Option<String>,
    /// Additional key-value details
    pub details: Vec<(String, String)>,
    /// Timestamp when the error occurred
    pub timestamp: u64,
}

impl ErrorContext {
    /// Create new error context (auto-captures timestamp)
    pub fn new(message: impl Into<String>) -> Self {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
        Self {
            message: message.into(),
            location: None,
            details: Vec::new(),
            timestamp: now,
        }
    }

    /// Add location information
    pub fn with_location(mut self, location: impl Into<String>) -> Self {
        self.location = Some(location.into());
        self
    }

    /// Add a key-value detail
    pub fn with_detail(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.details.push((key.into(), value.into()));
        self
    }
}

impl fmt::Display for ErrorContext {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message)?;
        if let Some(ref loc) = self.location {
            write!(f, " at {}", loc)?;
        }
        for (key, value) in &self.details {
            write!(f, " [{}={}]", key, value)?;
        }
        Ok(())
    }
}

// ============================================================================
// ContextualError - Error with attached context
// ============================================================================

/// An error with attached context information
#[derive(Debug)]
pub struct ContextualError {
    /// The underlying error
    pub error: GlobalError,
    /// Context providing additional information
    pub context: ErrorContext,
}

impl fmt::Display for ContextualError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} (context: {})", self.error, self.context)
    }
}

impl std::error::Error for ContextualError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.error)
    }
}

// ============================================================================
// WithContext extension trait
// ============================================================================

/// Extension trait to attach context to any `Result<T, GlobalError>`
pub trait WithContext<T> {
    /// Attach context to the error
    fn with_context(self, ctx: ErrorContext) -> Result<T, ContextualError>;

    /// Attach a simple message context to the error
    fn context(self, msg: impl Into<String>) -> Result<T, ContextualError>;
}

impl<T> WithContext<T> for Result<T, GlobalError> {
    fn with_context(self, ctx: ErrorContext) -> Result<T, ContextualError> {
        self.map_err(|err| ContextualError {
            error: err,
            context: ctx,
        })
    }

    fn context(self, msg: impl Into<String>) -> Result<T, ContextualError> {
        self.map_err(|err| ContextualError {
            error: err,
            context: ErrorContext::new(msg),
        })
    }
}

// ============================================================================
// Helper Macros
// ============================================================================

/// Create a formatted `GlobalError::Other`
#[macro_export]
macro_rules! format_err {
    ($msg:expr) => {
        $crate::agent::types::error::GlobalError::Other($msg.to_string())
    };
    ($fmt:expr, $($arg:tt)*) => {
        $crate::agent::types::error::GlobalError::Other(format!($fmt, $($arg)*))
    };
}

/// Create a `GlobalError::LLM`
#[macro_export]
macro_rules! llm_err {
    ($msg:expr) => {
        $crate::agent::types::error::GlobalError::LLM($msg.to_string())
    };
    ($fmt:expr, $($arg:tt)*) => {
        $crate::agent::types::error::GlobalError::LLM(format!($fmt, $($arg)*))
    };
}

/// Create a `GlobalError::Plugin`
#[macro_export]
macro_rules! plugin_err {
    ($msg:expr) => {
        $crate::agent::types::error::GlobalError::Plugin($msg.to_string())
    };
    ($fmt:expr, $($arg:tt)*) => {
        $crate::agent::types::error::GlobalError::Plugin(format!($fmt, $($arg)*))
    };
}

/// Create a `GlobalError::Config`
#[macro_export]
macro_rules! config_err {
    ($msg:expr) => {
        $crate::agent::types::error::GlobalError::Config($msg.to_string())
    };
    ($fmt:expr, $($arg:tt)*) => {
        $crate::agent::types::error::GlobalError::Config(format!($fmt, $($arg)*))
    };
}

/// Create a `GlobalError::Persistence`
#[macro_export]
macro_rules! persistence_err {
    ($msg:expr) => {
        $crate::agent::types::error::GlobalError::Persistence($msg.to_string())
    };
    ($fmt:expr, $($arg:tt)*) => {
        $crate::agent::types::error::GlobalError::Persistence(format!($fmt, $($arg)*))
    };
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_from_agent_error() {
        let agent_err = AgentError::NotFound("test-agent".to_string());
        let global_err: GlobalError = agent_err.into();

        assert_eq!(global_err.category(), ErrorCategory::Agent);
        assert!(global_err.to_string().contains("test-agent"));
    }

    #[test]
    fn test_error_categories() {
        assert_eq!(GlobalError::llm("test").category(), ErrorCategory::LLM);
        assert_eq!(
            GlobalError::plugin("test").category(),
            ErrorCategory::Plugin
        );
        assert_eq!(
            GlobalError::runtime("test").category(),
            ErrorCategory::Runtime
        );
    }

    #[test]
    fn test_new_error_categories() {
        assert_eq!(
            GlobalError::config("bad config").category(),
            ErrorCategory::Config
        );
        assert_eq!(
            GlobalError::persistence("connection lost").category(),
            ErrorCategory::Persistence
        );
        assert_eq!(
            GlobalError::prompt("missing variable").category(),
            ErrorCategory::Workflow
        );
        assert_eq!(
            GlobalError::dsl("invalid node").category(),
            ErrorCategory::Workflow
        );
        assert_eq!(
            GlobalError::wasm("compilation failed").category(),
            ErrorCategory::Plugin
        );
        assert_eq!(
            GlobalError::rhai("script error").category(),
            ErrorCategory::Plugin
        );
        assert_eq!(
            GlobalError::dora("node init failed").category(),
            ErrorCategory::Runtime
        );
        assert_eq!(
            GlobalError::message_graph("invalid edge").category(),
            ErrorCategory::MessageGraph
        );
    }

    #[test]
    fn test_retryable_errors() {
        assert!(GlobalError::llm("timeout").is_retryable());
        assert!(GlobalError::plugin("temporary failure").is_retryable());
        assert!(GlobalError::runtime("network error").is_retryable());
        assert!(GlobalError::wasm("timeout").is_retryable());
        assert!(GlobalError::dora("connection lost").is_retryable());
    }

    #[test]
    fn test_non_retryable_errors() {
        assert!(!GlobalError::config("invalid").is_retryable());
        assert!(!GlobalError::persistence("constraint violation").is_retryable());
        assert!(!GlobalError::prompt("missing var").is_retryable());
    }

    #[test]
    fn test_error_severity() {
        assert_eq!(
            GlobalError::llm("timeout").severity(),
            ErrorSeverity::Retryable
        );
        assert_eq!(
            GlobalError::config("invalid").severity(),
            ErrorSeverity::Error
        );

        let fatal = GlobalError::Agent(AgentError::InvalidStateTransition {
            from: "init".to_string(),
            to: "invalid".to_string(),
        });
        assert_eq!(fatal.severity(), ErrorSeverity::Fatal);
        assert!(fatal.is_fatal());
    }

    #[test]
    fn test_result_type() {
        type Result = GlobalResult<String>;

        let ok: Result = Ok("success".to_string());
        assert!(ok.is_ok());

        let err: Result = Err(GlobalError::llm("failed"));
        assert!(err.is_err());
    }

    #[test]
    fn test_error_context() {
        let ctx = ErrorContext::new("Something went wrong")
            .with_location("agent.rs:42")
            .with_detail("agent_id", "agent-1")
            .with_detail("attempt", "1");

        assert_eq!(ctx.message, "Something went wrong");
        assert_eq!(ctx.location, Some("agent.rs:42".to_string()));
        assert_eq!(ctx.details.len(), 2);
        assert!(ctx.timestamp > 0);
    }

    #[test]
    fn test_error_context_display() {
        let ctx = ErrorContext::new("oops")
            .with_location("main.rs:10")
            .with_detail("key", "value");
        let display = format!("{}", ctx);
        assert!(display.contains("oops"));
        assert!(display.contains("main.rs:10"));
        assert!(display.contains("key=value"));
    }

    #[test]
    fn test_with_context_trait() {
        let result: GlobalResult<()> = Err(GlobalError::llm("timeout"));
        let contextual = result.context("while calling OpenAI");

        assert!(contextual.is_err());
        let err = contextual.unwrap_err();
        assert!(err.to_string().contains("timeout"));
        assert!(err.to_string().contains("while calling OpenAI"));
    }

    #[test]
    fn test_contextual_error_source() {
        let result: GlobalResult<()> = Err(GlobalError::llm("timeout"));
        let contextual = result.context("calling API");
        let err = contextual.unwrap_err();

        // Verify error chain
        use std::error::Error;
        assert!(err.source().is_some());
    }

    #[test]
    fn test_macros() {
        let err = format_err!("test error");
        assert!(matches!(err, GlobalError::Other(_)));

        let err = llm_err!("LLM failed: {}", "timeout");
        assert!(matches!(err, GlobalError::LLM(_)));

        let err = plugin_err!("Plugin error: {}", "not found");
        assert!(matches!(err, GlobalError::Plugin(_)));

        let err = config_err!("bad config");
        assert!(matches!(err, GlobalError::Config(_)));

        let err = persistence_err!("db error: {}", "connection lost");
        assert!(matches!(err, GlobalError::Persistence(_)));
    }
}
