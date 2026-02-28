//! Agent 错误类型定义
//! Agent Error Type Definitions
//!
//! 统一的 Agent 错误处理
//! Unified error handling for Agents

use std::fmt;
use thiserror::Error;

/// Agent 操作结果类型
/// Agent operation result type
pub type AgentResult<T> = Result<T, AgentError>;

/// Agent 错误类型
/// Agent error types
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum AgentError {
    /// Agent 未找到
    /// Agent not found
    #[error("Agent not found: {0}")]
    NotFound(String),

    /// Agent 初始化失败
    /// Agent initialization failed
    #[error("Agent initialization failed: {0}")]
    InitializationFailed(String),

    #[error("Agent validation failed: {0}")]
    ValidationFailed(String),

    /// Agent 执行失败
    /// Agent execution failed
    #[error("Agent execution failed: {0}")]
    ExecutionFailed(String),

    /// 工具执行失败
    /// Tool execution failed
    #[error("Tool execution failed: {tool_name}: {message}")]
    ToolExecutionFailed { tool_name: String, message: String },

    /// 工具未找到
    /// Tool not found
    #[error("Tool not found: {0}")]
    ToolNotFound(String),

    /// 配置错误
    /// Configuration error
    #[error("Configuration error: {0}")]
    ConfigError(String),

    #[error("Shutdown Failed: {0}")]
    ShutdownFailed(String),
    /// 无效输入
    /// Invalid input
    #[error("Invalid input: {0}")]
    InvalidInput(String),

    /// 无效输出
    /// Invalid output
    #[error("Invalid output: {0}")]
    InvalidOutput(String),

    /// 状态错误
    /// State error
    #[error("Invalid state transition: from {from:?} to {to:?}")]
    InvalidStateTransition { from: String, to: String },

    /// 超时错误
    /// Timeout error
    #[error("Operation timed out after {duration_ms}ms")]
    Timeout { duration_ms: u64 },

    /// 中断错误
    /// Interruption error
    #[error("Operation was interrupted")]
    Interrupted,

    /// 资源不可用
    /// Resource unavailable
    #[error("Resource unavailable: {0}")]
    ResourceUnavailable(String),

    /// 能力不匹配
    /// Capability mismatch
    #[error("Capability mismatch: required {required}, available {available}")]
    CapabilityMismatch { required: String, available: String },

    /// 工厂未找到
    /// Factory not found
    #[error("Agent factory not found: {0}")]
    FactoryNotFound(String),

    /// 注册失败
    /// Registration failed
    #[error("Registration failed: {0}")]
    RegistrationFailed(String),

    /// 内存错误
    /// Memory error
    #[error("Memory error: {0}")]
    MemoryError(String),

    /// 推理错误
    /// Reasoning error
    #[error("Reasoning error: {0}")]
    ReasoningError(String),

    /// 协调错误
    /// Coordination error
    #[error("Coordination error: {0}")]
    CoordinationError(String),

    /// 序列化错误
    /// Serialization error
    #[error("Serialization error: {0}")]
    SerializationError(String),

    /// IO 错误
    /// IO error
    #[error("IO error: {0}")]
    IoError(String),

    /// 内部错误
    /// Internal error
    #[error("Internal error: {0}")]
    Internal(String),

    /// 其他错误
    /// Other errors
    #[error("{0}")]
    Other(String),
}

impl AgentError {
    /// 创建工具执行失败错误
    /// Create a tool execution failure error
    pub fn tool_execution_failed(tool_name: impl Into<String>, message: impl Into<String>) -> Self {
        Self::ToolExecutionFailed {
            tool_name: tool_name.into(),
            message: message.into(),
        }
    }

    /// 创建状态转换错误
    /// Create an invalid state transition error
    pub fn invalid_state_transition(from: impl fmt::Debug, to: impl fmt::Debug) -> Self {
        Self::InvalidStateTransition {
            from: format!("{:?}", from),
            to: format!("{:?}", to),
        }
    }

    /// 创建超时错误
    /// Create a timeout error
    pub fn timeout(duration_ms: u64) -> Self {
        Self::Timeout { duration_ms }
    }

    /// 创建能力不匹配错误
    /// Create a capability mismatch error
    pub fn capability_mismatch(required: impl Into<String>, available: impl Into<String>) -> Self {
        Self::CapabilityMismatch {
            required: required.into(),
            available: available.into(),
        }
    }

    /// Returns `true` for transient errors, `false` for permanent ones.
    pub fn is_retryable(&self) -> bool {
        matches!(
            self,
            AgentError::Timeout { .. }
                | AgentError::ResourceUnavailable(_)
                | AgentError::ExecutionFailed(_)
                | AgentError::ToolExecutionFailed { .. }
                | AgentError::CoordinationError(_)
                | AgentError::Internal(_)
                | AgentError::IoError(_)
                | AgentError::ReasoningError(_)
                | AgentError::MemoryError(_)
        )
    }

    /// 此错误是否为瞬态错误，操作可能在重试时成功
    /// Whether this error is transient and the operation may succeed on retry.
    ///
    /// 瞬态错误包括超时、资源不可用、IO 失败和通用执行失败。
    /// 永久错误（配置、未找到、无效输入、中断、验证）应快速失败。
    /// Transient errors include timeouts, resource unavailability, IO failures,
    /// and generic execution failures. Permanent errors (config, not-found,
    /// invalid input, interruption, validation) should fail fast.
    ///
    /// NOTE: This is stricter than [`is_retryable`] — it excludes `Internal`,
    /// `ToolExecutionFailed`, `ReasoningError`, and `MemoryError` which may
    /// indicate bugs rather than transient infrastructure issues.
    pub fn is_transient(&self) -> bool {
        matches!(
            self,
            Self::Timeout { .. }
                | Self::ResourceUnavailable(_)
                | Self::IoError(_)
                | Self::ExecutionFailed(_)
                | Self::CoordinationError(_)
        )
    }
}

impl From<std::io::Error> for AgentError {
    fn from(err: std::io::Error) -> Self {
        AgentError::IoError(err.to_string())
    }
}

impl From<serde_json::Error> for AgentError {
    fn from(err: serde_json::Error) -> Self {
        AgentError::SerializationError(err.to_string())
    }
}

impl From<crate::plugin::PluginError> for AgentError {
    fn from(err: crate::plugin::PluginError) -> Self {
        AgentError::Internal(err.to_string())
    }
}

impl From<crate::bus::BusError> for AgentError {
    fn from(err: crate::bus::BusError) -> Self {
        AgentError::Internal(err.to_string())
    }
}

impl From<crate::agent::secretary::ConnectionError> for AgentError {
    fn from(err: crate::agent::secretary::ConnectionError) -> Self {
        AgentError::Internal(err.to_string())
    }
}

impl From<crate::agent::secretary::SecretaryError> for AgentError {
    fn from(err: crate::agent::secretary::SecretaryError) -> Self {
        AgentError::Internal(err.to_string())
    }
}

#[cfg(feature = "config")]
impl From<crate::config::ConfigError> for AgentError {
    fn from(err: crate::config::ConfigError) -> Self {
        match err {
            crate::config::ConfigError::Io(e) => {
                AgentError::ConfigError(format!("Failed to read config file: {}", e))
            }
            crate::config::ConfigError::Parse(e) => {
                AgentError::ConfigError(format!("Failed to parse config: {}", e))
            }
            crate::config::ConfigError::UnsupportedFormat(e) => {
                AgentError::ConfigError(format!("Unsupported config format: {}", e))
            }
            crate::config::ConfigError::Serialization(e) => {
                AgentError::ConfigError(format!("Failed to deserialize config: {}", e))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_display() {
        /// 测试错误显示
        /// Test error display
        let err = AgentError::NotFound("test-agent".to_string());
        assert_eq!(err.to_string(), "Agent not found: test-agent");
    }

    #[test]
    fn test_tool_execution_failed() {
        /// 测试工具执行失败
        /// Test tool execution failure
        let err = AgentError::tool_execution_failed("calculator", "division by zero");
        assert!(err.to_string().contains("calculator"));
        assert!(err.to_string().contains("division by zero"));
    }

    #[cfg(feature = "config")]
    #[test]
    fn config_error_converts_via_from() {
        let config_err = crate::config::ConfigError::Parse("bad yaml".into());
        let agent_err: AgentError = config_err.into();

        assert!(matches!(agent_err, AgentError::ConfigError(_)));
        if let AgentError::ConfigError(msg) = agent_err {
            assert!(msg.contains("bad yaml"));
        }
    }

    #[test]
    fn test_is_transient_classification() {
        // Transient errors — should be retried
        assert!(AgentError::Timeout { duration_ms: 5000 }.is_transient());
        assert!(AgentError::ResourceUnavailable("gpu".into()).is_transient());
        assert!(AgentError::IoError("connection reset".into()).is_transient());
        assert!(AgentError::ExecutionFailed("LLM timeout".into()).is_transient());
        assert!(AgentError::CoordinationError("peer unreachable".into()).is_transient());

        // Permanent errors — should NOT be retried
        assert!(!AgentError::ConfigError("bad yaml".into()).is_transient());
        assert!(!AgentError::NotFound("agent-x".into()).is_transient());
        assert!(!AgentError::InvalidInput("missing field".into()).is_transient());
        assert!(!AgentError::Interrupted.is_transient());
        assert!(!AgentError::ValidationFailed("schema".into()).is_transient());
        assert!(!AgentError::Internal("panic".into()).is_transient());
    }
}
