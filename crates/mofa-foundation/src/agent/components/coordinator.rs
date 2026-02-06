//! 协调组件
//!
//! 从 kernel 层导入 Coordinator trait，提供具体实现

use mofa_kernel::agent::components::coordinator::{
    aggregate_outputs, AggregationStrategy, Coordinator, CoordinationPattern, DispatchResult, Task,
};
use mofa_kernel::agent::context::CoreAgentContext;
use mofa_kernel::agent::types::AgentOutput;
use mofa_kernel::agent::AgentResult;
use async_trait::async_trait;

// ============================================================================
// 具体协调器实现
// ============================================================================

/// 顺序协调器
///
/// 按顺序将任务分发给多个 Agent
pub struct SequentialCoordinator {
    agent_ids: Vec<String>,
}

impl SequentialCoordinator {
    /// 创建新的顺序协调器
    pub fn new(agent_ids: Vec<String>) -> Self {
        Self { agent_ids }
    }
}

#[async_trait]
impl Coordinator for SequentialCoordinator {
    async fn dispatch(&self, task: Task, _ctx: &CoreAgentContext) -> AgentResult<Vec<DispatchResult>> {
        // 简化实现：为每个 agent 创建待处理结果
        let mut results = Vec::new();
        for agent_id in &self.agent_ids {
            results.push(DispatchResult::pending(&task.id, agent_id));
        }
        Ok(results)
    }

    async fn aggregate(&self, results: Vec<AgentOutput>) -> AgentResult<AgentOutput> {
        let texts: Vec<String> = results.iter().map(|o| o.to_text()).collect();
        Ok(AgentOutput::text(texts.join("\n\n---\n\n")))
    }

    fn pattern(&self) -> CoordinationPattern {
        CoordinationPattern::Sequential
    }

    fn name(&self) -> &str {
        "sequential"
    }

    async fn select_agents(&self, _task: &Task, _ctx: &CoreAgentContext) -> AgentResult<Vec<String>> {
        Ok(self.agent_ids.clone())
    }
}

/// 并行协调器
///
/// 并行将任务分发给多个 Agent
pub struct ParallelCoordinator {
    agent_ids: Vec<String>,
}

impl ParallelCoordinator {
    /// 创建新的并行协调器
    pub fn new(agent_ids: Vec<String>) -> Self {
        Self { agent_ids }
    }
}

#[async_trait]
impl Coordinator for ParallelCoordinator {
    async fn dispatch(&self, task: Task, _ctx: &CoreAgentContext) -> AgentResult<Vec<DispatchResult>> {
        let mut results = Vec::new();
        for agent_id in &self.agent_ids {
            results.push(DispatchResult::pending(&task.id, agent_id));
        }
        Ok(results)
    }

    async fn aggregate(&self, results: Vec<AgentOutput>) -> AgentResult<AgentOutput> {
        aggregate_outputs(results, &AggregationStrategy::CollectAll)
    }

    fn pattern(&self) -> CoordinationPattern {
        CoordinationPattern::Parallel
    }

    fn name(&self) -> &str {
        "parallel"
    }

    async fn select_agents(&self, _task: &Task, _ctx: &CoreAgentContext) -> AgentResult<Vec<String>> {
        Ok(self.agent_ids.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sequential_coordinator() {
        let coordinator = SequentialCoordinator::new(vec!["agent-1".to_string(), "agent-2".to_string()]);
        assert_eq!(coordinator.name(), "sequential");
        assert_eq!(coordinator.pattern(), CoordinationPattern::Sequential);
    }

    #[test]
    fn test_parallel_coordinator() {
        let coordinator = ParallelCoordinator::new(vec!["agent-1".to_string(), "agent-2".to_string()]);
        assert_eq!(coordinator.name(), "parallel");
        assert_eq!(coordinator.pattern(), CoordinationPattern::Parallel);
    }

    #[tokio::test]
    async fn test_sequential_dispatch() {
        let coordinator = SequentialCoordinator::new(vec!["agent-1".to_string(), "agent-2".to_string()]);
        let ctx = CoreAgentContext::new("test");
        let task = Task::new("task-1", "Do something");

        let results = coordinator.dispatch(task, &ctx).await.unwrap();
        assert_eq!(results.len(), 2);
    }

    #[tokio::test]
    async fn test_sequential_aggregate() {
        let coordinator = SequentialCoordinator::new(vec!["agent-1".to_string()]);
        let results = vec![
            AgentOutput::text("Result 1"),
            AgentOutput::text("Result 2"),
        ];

        let aggregated = coordinator.aggregate(results).await.unwrap();
        assert!(aggregated.to_text().contains("Result 1"));
        assert!(aggregated.to_text().contains("Result 2"));
    }
}
