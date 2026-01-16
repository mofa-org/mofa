//! Agent 辅助类型定义
//!
//! 提供元数据、统计信息等辅助类型

use super::core::MoFAAgent;
use super::types::AgentState;
use std::sync::Arc;
use tokio::sync::RwLock;
use crate::agent::{AgentCapabilities, AgentContext, AgentResult, InterruptResult};

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
#[derive(Debug, Clone, PartialEq, Eq)]
#[derive(Default)]
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
// 基础 Agent 实现
// ============================================================================

/// 基础 Agent 实现
///
/// 提供 Agent 的基础功能，可以被继承或组合
pub struct BaseAgent {
    /// Agent ID
    pub id: String,
    /// Agent 名称
    pub name: String,
    /// Agent 描述
    pub description: Option<String>,
    /// Agent 版本
    pub version: Option<String>,
    /// Agent 能力
    pub capabilities: AgentCapabilities,
    /// 当前状态
    pub state: AgentState,
    /// 统计信息
    stats: AgentStats,
}

impl BaseAgent {
    /// 创建新的基础 Agent
    pub fn new(id: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            description: None,
            version: None,
            capabilities: AgentCapabilities::default(),
            state: AgentState::Created,
            stats: AgentStats::default(),
        }
    }

    /// 设置描述
    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// 设置版本
    pub fn with_version(mut self, version: impl Into<String>) -> Self {
        self.version = Some(version.into());
        self
    }

    /// 设置能力
    pub fn with_capabilities(mut self, capabilities: AgentCapabilities) -> Self {
        self.capabilities = capabilities;
        self
    }

    /// 转换状态
    pub fn transition_to(&mut self, new_state: AgentState) -> AgentResult<()> {
        if self.state.can_transition_to(&new_state) {
            self.state = new_state;
            Ok(())
        } else {
            Err(super::error::AgentError::invalid_state_transition(&self.state, &new_state))
        }
    }

    /// 记录成功执行
    pub fn record_success(&mut self, duration_ms: u64, tokens: u64, tool_calls: u64) {
        self.stats.total_executions += 1;
        self.stats.successful_executions += 1;
        self.stats.total_tokens_used += tokens;
        self.stats.total_tool_calls += tool_calls;

        // 更新平均执行时间
        let n = self.stats.total_executions as f64;
        self.stats.avg_execution_time_ms =
            (self.stats.avg_execution_time_ms * (n - 1.0) + duration_ms as f64) / n;
    }

    /// 记录失败执行
    pub fn record_failure(&mut self) {
        self.stats.total_executions += 1;
        self.stats.failed_executions += 1;
    }

    /// 获取统计信息
    pub fn stats(&self) -> &AgentStats {
        &self.stats
    }

    /// 获取 ID
    pub fn id(&self) -> &str {
        &self.id
    }

    /// 获取名称
    pub fn name(&self) -> &str {
        &self.name
    }

    /// 获取能力
    pub fn capabilities(&self) -> &AgentCapabilities {
        &self.capabilities
    }

    /// 获取状态
    pub fn state(&self) -> AgentState {
        self.state.clone()
    }

    /// 初始化
    pub async fn initialize(&mut self, _ctx: &AgentContext) -> AgentResult<()> {
        self.transition_to(AgentState::Initializing)?;
        self.transition_to(AgentState::Ready)?;
        Ok(())
    }

    /// 中断
    pub async fn interrupt(&mut self) -> AgentResult<InterruptResult> {
        Ok(InterruptResult::Acknowledged)
    }

    /// 关闭
    pub async fn shutdown(&mut self) -> AgentResult<()> {
        self.transition_to(AgentState::ShuttingDown)?;
        self.transition_to(AgentState::Shutdown)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use async_trait::async_trait;
    use crate::agent::{AgentInput, AgentOutput};
    use super::*;

    struct TestAgent {
        base: BaseAgent,
    }

    impl TestAgent {
        fn new() -> Self {
            Self {
                base: BaseAgent::new("test-agent", "Test Agent")
                    .with_description("A test agent")
                    .with_version("1.0.0"),
            }
        }
    }

    #[async_trait]
    impl MoFAAgent for TestAgent {
        fn id(&self) -> &str {
            &self.base.id
        }

        fn name(&self) -> &str {
            &self.base.name
        }

        fn capabilities(&self) -> &AgentCapabilities {
            &self.base.capabilities
        }

        async fn initialize(&mut self, _ctx: &AgentContext) -> AgentResult<()> {
            self.base.transition_to(AgentState::Initializing)?;
            self.base.transition_to(AgentState::Ready)?;
            Ok(())
        }

        async fn execute(
            &mut self,
            input: AgentInput,
            _ctx: &AgentContext,
        ) -> AgentResult<AgentOutput> {
            self.base.transition_to(AgentState::Executing)?;
            let result = AgentOutput::text(format!("Received: {}", input.to_text()));
            self.base.transition_to(AgentState::Ready)?;
            self.base.record_success(100, 50, 0);
            Ok(result)
        }

        async fn shutdown(&mut self) -> AgentResult<()> {
            self.base.transition_to(AgentState::ShuttingDown)?;
            self.base.transition_to(AgentState::Shutdown)?;
            Ok(())
        }

        fn state(&self) -> AgentState {
            self.base.state.clone()
        }
    }

    #[tokio::test]
    async fn test_agent_lifecycle() {
        let mut agent = TestAgent::new();
        let ctx = AgentContext::new("test-exec");

        assert_eq!(agent.state(), AgentState::Created);

        agent.initialize(&ctx).await.unwrap();
        assert_eq!(agent.state(), AgentState::Ready);

        let output = agent.execute(AgentInput::text("Hello"), &ctx).await.unwrap();
        assert!(output.to_text().contains("Hello"));
        assert_eq!(agent.state(), AgentState::Ready);

        agent.shutdown().await.unwrap();
        assert_eq!(agent.state(), AgentState::Shutdown);
    }

    #[tokio::test]
    async fn test_agent_stats() {
        let mut agent = TestAgent::new();
        let ctx = AgentContext::new("test-exec");

        agent.initialize(&ctx).await.unwrap();
        agent.execute(AgentInput::text("1"), &ctx).await.unwrap();
        agent.execute(AgentInput::text("2"), &ctx).await.unwrap();

        let stats = agent.base.stats();
        assert_eq!(stats.total_executions, 2);
        assert_eq!(stats.successful_executions, 2);
    }

    #[test]
    fn test_agent_metadata() {
        let agent = TestAgent::new();
        let metadata = AgentMetadata::from_agent(&agent);

        assert_eq!(metadata.id, "test-agent");
        assert_eq!(metadata.name, "Test Agent");
        // from_agent 不会自动设置 description，所以这里是 None
        assert_eq!(metadata.description, None);
    }
}
