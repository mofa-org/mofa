//! Agent 辅助类型定义
//!
//! 提供元数据、统计信息等辅助类型

use super::core::MoFAAgent;
use super::types::AgentState;
use std::sync::Arc;
use tokio::sync::RwLock;

/// 动态分发的 MoFAAgent
///
/// # 注意
///
/// 之前称为 `DynAgent` 基于 `AgentCore`，现在基于统一的 `MoFAAgent`。
pub type DynAgent = Arc<RwLock<dyn MoFAAgent>>;

// ============================================================================
// 辅助类型
// ============================================================================

/// Agent 健康状态
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum HealthStatus {
    /// 健康
    #[default]
    Healthy,
    /// 降级 (部分功能不可用)
    Degraded(String),
    /// 不健康
    Unhealthy(String),
}

/// Agent 统计信息
#[derive(Debug, Clone, Default)]
pub struct AgentStats {
    /// 总执行次数
    pub total_executions: u64,
    /// 成功次数
    pub successful_executions: u64,
    /// 失败次数
    pub failed_executions: u64,
    /// 平均执行时间 (毫秒)
    pub avg_execution_time_ms: f64,
    /// 总 Token 使用
    pub total_tokens_used: u64,
    /// 总工具调用次数
    pub total_tool_calls: u64,
}

// ============================================================================
// Agent 元数据
// ============================================================================

/// Agent 元数据
#[derive(Debug, Clone)]
pub struct AgentMetadata {
    /// Agent ID
    pub id: String,
    /// Agent 名称
    pub name: String,
    /// Agent 描述
    pub description: Option<String>,
    /// Agent 版本
    pub version: Option<String>,
    /// Agent 能力
    pub capabilities: crate::agent::capabilities::AgentCapabilities,
    /// Agent 状态
    pub state: AgentState,
}

impl AgentMetadata {
    /// 从 MoFAAgent 创建元数据
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
