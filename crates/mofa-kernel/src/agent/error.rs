//! Agent 错误类型定义
//!
//! 统一的 Agent 错误处理

use std::fmt;
use thiserror::Error;

/// Agent 操作结果类型
pub type AgentResult<T> = Result<T, AgentError>;

/// Agent 错误类型
#[derive(Debug, Error)]
pub enum AgentError {
    /// Agent 未找到
    #[error("Agent not found: {0}")]
    NotFound(String),

    /// Agent 初始化失败
    #[error("Agent initialization failed: {0}")]
    InitializationFailed(String),

    #[error("Agent validation failed: {0}")]
    ValidationFailed(String),

    /// Agent 执行失败
    #[error("Agent execution failed: {0}")]
    ExecutionFailed(String),

    /// 工具执行失败
    #[error("Tool execution failed: {tool_name}: {message}")]
    ToolExecutionFailed { tool_name: String, message: String },

    /// 工具未找到
    #[error("Tool not found: {0}")]
    ToolNotFound(String),

    /// 配置错误
    #[error("Configuration error: {0}")]
    ConfigError(String),

    #[error("Shutdown Failed: {0}")]
    ShutdownFailed(String),
    /// 无效输入
    #[error("Invalid input: {0}")]
    InvalidInput(String),

    /// 无效输出
    #[error("Invalid output: {0}")]
    InvalidOutput(String),

    /// 状态错误
    #[error("Invalid state transition: from {from:?} to {to:?}")]
    InvalidStateTransition { from: String, to: String },

    /// 超时错误
    #[error("Operation timed out after {duration_ms}ms")]
    Timeout { duration_ms: u64 },

    /// 中断错误
    #[error("Operation was interrupted")]
    Interrupted,

    /// 资源不可用
    #[error("Resource unavailable: {0}")]
    ResourceUnavailable(String),

    /// 能力不匹配
    #[error("Capability mismatch: required {required}, available {available}")]
    CapabilityMismatch { required: String, available: String },

    /// 工厂未找到
    #[error("Agent factory not found: {0}")]
    FactoryNotFound(String),

    /// 注册失败
    #[error("Registration failed: {0}")]
    RegistrationFailed(String),

    /// 内存错误
    #[error("Memory error: {0}")]
    MemoryError(String),

    /// 推理错误
    #[error("Reasoning error: {0}")]
    ReasoningError(String),

    /// 协调错误
    #[error("Coordination error: {0}")]
    CoordinationError(String),

    /// 序列化错误
    #[error("Serialization error: {0}")]
    SerializationError(String),

    /// IO 错误
    #[error("IO error: {0}")]
    IoError(String),

    /// 内部错误
    #[error("Internal error: {0}")]
    Internal(String),

    /// 其他错误
    #[error("{0}")]
    Other(String),
}

impl AgentError {
    /// 创建工具执行失败错误
    pub fn tool_execution_failed(tool_name: impl Into<String>, message: impl Into<String>) -> Self {
        Self::ToolExecutionFailed {
            tool_name: tool_name.into(),
            message: message.into(),
        }
    }

    /// 创建状态转换错误
    pub fn invalid_state_transition(from: impl fmt::Debug, to: impl fmt::Debug) -> Self {
        Self::InvalidStateTransition {
            from: format!("{:?}", from),
            to: format!("{:?}", to),
        }
    }

    /// 创建超时错误
    pub fn timeout(duration_ms: u64) -> Self {
        Self::Timeout { duration_ms }
    }

    /// 创建能力不匹配错误
    pub fn capability_mismatch(required: impl Into<String>, available: impl Into<String>) -> Self {
        Self::CapabilityMismatch {
            required: required.into(),
            available: available.into(),
        }
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

impl From<anyhow::Error> for AgentError {
    fn from(err: anyhow::Error) -> Self {
        AgentError::Internal(err.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_display() {
        let err = AgentError::NotFound("test-agent".to_string());
        assert_eq!(err.to_string(), "Agent not found: test-agent");
    }

    #[test]
    fn test_tool_execution_failed() {
        let err = AgentError::tool_execution_failed("calculator", "division by zero");
        assert!(err.to_string().contains("calculator"));
        assert!(err.to_string().contains("division by zero"));
    }
}
