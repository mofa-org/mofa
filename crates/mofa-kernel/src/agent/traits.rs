//! Agent 辅助类型定义
//! Agent auxiliary type definitions
//!
//! 提供元数据、统计信息等辅助类型
//! Provides auxiliary types such as metadata and statistical information

use super::core::MoFAAgent;
use super::types::AgentState;
use std::sync::Arc;
use tokio::sync::RwLock;

/// 动态分发的 MoFAAgent
/// Dynamically dispatched MoFAAgent
///
/// # 注意
/// # Note
///
/// 之前称为 `DynAgent` 基于 `AgentCore`，现在基于统一的 `MoFAAgent`。
/// Previously called `DynAgent` based on `AgentCore`, now based on the unified `MoFAAgent`.
pub type DynAgent = Arc<RwLock<dyn MoFAAgent>>;

// ============================================================================
// 辅助类型
// Auxiliary types
// ============================================================================

/// Agent 健康状态
/// Agent health status
#[derive(Debug, Clone, PartialEq, Eq, Default)]
#[non_exhaustive]
pub enum HealthStatus {
    /// 健康
    /// Healthy
    #[default]
    Healthy,
    /// 降级 (部分功能不可用)
    /// Degraded (Partial functionality unavailable)
    Degraded(String),
    /// 不健康
    /// Unhealthy
    Unhealthy(String),
}

/// Agent 统计信息
/// Agent statistical information
#[derive(Debug, Clone, Default)]
pub struct AgentStats {
    /// 总执行次数
    /// Total execution count
    pub total_executions: u64,
    /// 成功次数
    /// Number of successes
    pub successful_executions: u64,
    /// 失败次数
    /// Number of failures
    pub failed_executions: u64,
    /// 平均执行时间 (毫秒)
    /// Average execution time (ms)
    pub avg_execution_time_ms: f64,
    /// 总 Token 使用
    /// Total tokens used
    pub total_tokens_used: u64,
    /// 总工具调用次数
    /// Total tool call count
    pub total_tool_calls: u64,
}

// ============================================================================
// Agent 元数据
// Agent metadata
// ============================================================================

/// Agent 元数据
/// Agent metadata
#[derive(Debug, Clone)]
pub struct AgentMetadata {
    /// Agent ID
    /// Agent ID
    pub id: String,
    /// Agent 名称
    /// Agent name
    pub name: String,
    /// Agent 描述
    /// Agent description
    pub description: Option<String>,
    /// Agent 版本
    /// Agent version
    pub version: Option<String>,
    /// Agent 能力
    /// Agent capabilities
    pub capabilities: crate::agent::capabilities::AgentCapabilities,
    /// Agent 状态
    /// Agent state
    pub state: AgentState,
}

impl AgentMetadata {
    /// 从 MoFAAgent 创建元数据
    /// Create metadata from MoFAAgent
    pub fn from_agent(agent: &dyn MoFAAgent) -> Self {
        Self {
            id: agent.id().to_string(),
            name: agent.name().to_string(),
            description: None,
            version: None,
            capabilities: agent.capabilities().clone(),
            state: agent.state(),
        }
    }
}

// ============================================================================
// Note: BaseAgent implementation has been moved to mofa-foundation/src/agent/base.rs
// This file now only contains trait definitions and helper types
// Kernel tests use inline mock implementations instead of BaseAgent
// ============================================================================
