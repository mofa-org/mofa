//! 全局错误类型系统
//!
//! 本模块提供全局错误类型层次结构，整合各层的错误类型。
//!
//! # 设计目标
//!
//! - 提供全局错误抽象，避免多种错误类型（AgentResult, LLMResult, PluginResult）
//! - 保留各层错误的特定信息
//! - 支持错误链和上下文
//! - 提供清晰的错误来源标识

use crate::agent::error::AgentError;
use std::fmt;

// ============================================================================
// GlobalError - 全局错误类型
// ============================================================================

/// 全局错误类型
///
/// 整合所有层的错误类型，提供单一的错误抽象。
#[derive(Debug, thiserror::Error)]
pub enum GlobalError {
    /// Agent 层错误
    #[error("Agent error: {0}")]
    Agent(#[from] AgentError),

    /// LLM 层错误
    #[error("LLM error: {0}")]
    LLM(String),

    /// 插件错误
    #[error("Plugin error: {0}")]
    Plugin(String),

    /// 运行时错误
    #[error("Runtime error: {0}")]
    Runtime(String),

    /// IO 错误
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// 序列化错误
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    /// 其他错误
    #[error("Other error: {0}")]
    Other(String),
}

impl GlobalError {
    /// 创建 LLM 错误
    pub fn llm(msg: impl Into<String>) -> Self {
        Self::LLM(msg.into())
    }

    /// 创建插件错误
    pub fn plugin(msg: impl Into<String>) -> Self {
        Self::Plugin(msg.into())
    }

    /// 创建运行时错误
    pub fn runtime(msg: impl Into<String>) -> Self {
        Self::Runtime(msg.into())
    }

    /// 获取错误分类
    pub fn category(&self) -> ErrorCategory {
        match self {
            Self::Agent(_) => ErrorCategory::Agent,
            Self::LLM(_) => ErrorCategory::LLM,
            Self::Plugin(_) => ErrorCategory::Plugin,
            Self::Runtime(_) => ErrorCategory::Runtime,
            Self::Io(_) => ErrorCategory::Io,
            Self::Serialization(_) => ErrorCategory::Serialization,
            Self::Other(_) => ErrorCategory::Other,
        }
    }

    /// 是否为可重试错误
    pub fn is_retryable(&self) -> bool {
        matches!(
            self,
            Self::Runtime(_) | Self::Io(_) | Self::LLM(_) | Self::Plugin(_)
        )
    }

    /// 是否为致命错误
    pub fn is_fatal(&self) -> bool {
        matches!(self, Self::Agent(AgentError::InvalidStateTransition { .. }))
    }
}

// ============================================================================
// ErrorCategory - 错误分类
// ============================================================================

/// 错误分类
///
/// 用于错误统计和监控。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ErrorCategory {
    /// Agent 相关错误
    Agent,
    /// LLM 相关错误
    LLM,
    /// 插件相关错误
    Plugin,
    /// 运行时错误
    Runtime,
    /// IO 错误
    Io,
    /// 序列化错误
    Serialization,
    /// 其他错误
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
            Self::Other => write!(f, "other"),
        }
    }
}

// ============================================================================
// GlobalResult - 全局结果类型
// ============================================================================

/// 全局结果类型
///
/// 替代 `AgentResult`, `LLMResult`, `PluginResult`，提供单一的结果类型。
pub type GlobalResult<T> = Result<T, GlobalError>;

// ============================================================================
// 从其他错误类型转换
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
// ErrorContext - 错误上下文
// ============================================================================

/// 错误上下文
///
/// 用于携带额外的错误信息。
#[derive(Debug, Clone)]
pub struct ErrorContext {
    /// 错误消息
    pub message: String,
    /// 错误位置
    pub location: Option<String>,
    /// 附加信息
    pub details: Vec<(String, String)>,
}

impl ErrorContext {
    /// 创建新的错误上下文
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            location: None,
            details: Vec::new(),
        }
    }

    /// 添加位置信息
    pub fn with_location(mut self, location: impl Into<String>) -> Self {
        self.location = Some(location.into());
        self
    }

    /// 添加详细信息
    pub fn with_detail(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.details.push((key.into(), value.into()));
        self
    }
}

// ============================================================================
// 辅助宏
// ============================================================================

/// 创建格式化错误
#[macro_export]
macro_rules! format_err {
    ($msg:expr) => {
        $crate::agent::types::error::GlobalError::Other($msg.to_string())
    };
    ($fmt:expr, $($arg:tt)*) => {
        $crate::agent::types::error::GlobalError::Other(format!($fmt, $($arg)*))
    };
}

/// 创建 LLM 错误
#[macro_export]
macro_rules! llm_err {
    ($msg:expr) => {
        $crate::agent::types::error::GlobalError::LLM($msg.to_string())
    };
    ($fmt:expr, $($arg:tt)*) => {
        $crate::agent::types::error::GlobalError::LLM(format!($fmt, $($arg)*))
    };
}

/// 创建插件错误
#[macro_export]
macro_rules! plugin_err {
    ($msg:expr) => {
        $crate::agent::types::error::GlobalError::Plugin($msg.to_string())
    };
    ($fmt:expr, $($arg:tt)*) => {
        $crate::agent::types::error::GlobalError::Plugin(format!($fmt, $($arg)*))
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
        assert_eq!(GlobalError::plugin("test").category(), ErrorCategory::Plugin);
        assert_eq!(GlobalError::runtime("test").category(), ErrorCategory::Runtime);
    }

    #[test]
    fn test_retryable_errors() {
        assert!(GlobalError::llm("timeout").is_retryable());
        assert!(GlobalError::plugin("temporary failure").is_retryable());
        assert!(GlobalError::runtime("network error").is_retryable());
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
    }

    #[test]
    fn test_macros() {
        let err = format_err!("test error");
        assert!(matches!(err, GlobalError::Other(_)));

        let err = llm_err!("LLM failed: {}", "timeout");
        assert!(matches!(err, GlobalError::LLM(_)));

        let err = plugin_err!("Plugin error: {}", "not found");
        assert!(matches!(err, GlobalError::Plugin(_)));
    }
}
