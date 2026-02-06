//! 协调组件
//!
//! 定义多 Agent 协调能力

use crate::agent::context::AgentContext;
use crate::agent::error::AgentResult;
use crate::agent::types::AgentOutput;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// 协调器 Trait
///
/// 负责多 Agent 的任务分发和结果聚合
///
/// # 示例
///
/// ```rust,ignore
/// use mofa_kernel::agent::components::coordinator::{Coordinator, CoordinationPattern, Task, DispatchResult};
///
/// struct SequentialCoordinator {
///     agent_ids: Vec<String>,
/// }
///
/// #[async_trait]
/// impl Coordinator for SequentialCoordinator {
///     async fn dispatch(&self, task: Task, ctx: &CoreAgentContext) -> AgentResult<Vec<DispatchResult>> {
///         // Sequential dispatch implementation
///     }
///
///     async fn aggregate(&self, results: Vec<AgentOutput>) -> AgentResult<AgentOutput> {
///         // Combine results
///     }
///
///     fn pattern(&self) -> CoordinationPattern {
///         CoordinationPattern::Sequential
///     }
/// }
/// ```
#[async_trait]
pub trait Coordinator: Send + Sync {
    /// 分发任务给 Agent(s)
    async fn dispatch(&self, task: Task, ctx: &AgentContext) -> AgentResult<Vec<DispatchResult>>;

    /// 聚合多个 Agent 的结果
    async fn aggregate(&self, results: Vec<AgentOutput>) -> AgentResult<AgentOutput>;

    /// 获取协调模式
    fn pattern(&self) -> CoordinationPattern;

    /// 协调器名称
    fn name(&self) -> &str {
        "coordinator"
    }

    /// 选择执行任务的 Agent
    async fn select_agents(&self, task: &Task, ctx: &AgentContext) -> AgentResult<Vec<String>> {
        let _ = (task, ctx);
        Ok(vec![])
    }

    /// 是否需要所有 Agent 完成
    fn requires_all(&self) -> bool {
        matches!(
            self.pattern(),
            CoordinationPattern::Parallel | CoordinationPattern::Consensus { .. }
        )
    }
}

/// 协调模式
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub enum CoordinationPattern {
    /// 顺序执行
    #[default]
    Sequential,
    /// 并行执行
    Parallel,
    /// 层级执行 (带监督者)
    Hierarchical {
        /// 监督者 Agent ID
        supervisor_id: String,
    },
    /// 共识模式 (需要达成一致)
    Consensus {
        /// 共识阈值 (0.0 - 1.0)
        threshold: f32,
    },
    /// 辩论模式
    Debate {
        /// 最大轮次
        max_rounds: usize,
    },
    /// MapReduce 模式
    MapReduce,
    /// 投票模式
    Voting,
    /// 自定义模式
    Custom(String),
}

/// 任务定义
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Task {
    /// 任务 ID
    pub id: String,
    /// 任务类型
    pub task_type: TaskType,
    /// 任务内容
    pub content: String,
    /// 任务优先级
    pub priority: TaskPriority,
    /// 目标 Agent ID (可选，如果为空则由协调器选择)
    pub target_agent: Option<String>,
    /// 任务参数
    pub params: HashMap<String, serde_json::Value>,
    /// 任务元数据
    pub metadata: HashMap<String, String>,
    /// 创建时间
    pub created_at: u64,
    /// 超时时间 (毫秒)
    pub timeout_ms: Option<u64>,
}

impl Task {
    /// 创建新任务
    pub fn new(id: impl Into<String>, content: impl Into<String>) -> Self {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;

        Self {
            id: id.into(),
            task_type: TaskType::General,
            content: content.into(),
            priority: TaskPriority::Normal,
            target_agent: None,
            params: HashMap::new(),
            metadata: HashMap::new(),
            created_at: now,
            timeout_ms: None,
        }
    }

    /// 设置任务类型
    pub fn with_type(mut self, task_type: TaskType) -> Self {
        self.task_type = task_type;
        self
    }

    /// 设置优先级
    pub fn with_priority(mut self, priority: TaskPriority) -> Self {
        self.priority = priority;
        self
    }

    /// 设置目标 Agent
    pub fn for_agent(mut self, agent_id: impl Into<String>) -> Self {
        self.target_agent = Some(agent_id.into());
        self
    }

    /// 添加参数
    pub fn with_param(mut self, key: impl Into<String>, value: serde_json::Value) -> Self {
        self.params.insert(key.into(), value);
        self
    }

    /// 设置超时
    pub fn with_timeout(mut self, timeout_ms: u64) -> Self {
        self.timeout_ms = Some(timeout_ms);
        self
    }
}

/// 任务类型
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TaskType {
    /// 通用任务
    General,
    /// 分析任务
    Analysis,
    /// 生成任务
    Generation,
    /// 审查任务
    Review,
    /// 决策任务
    Decision,
    /// 搜索任务
    Search,
    /// 自定义任务
    Custom(String),
}

/// 任务优先级
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, Default)]
pub enum TaskPriority {
    Low = 0,
    #[default]
    Normal = 1,
    High = 2,
    Urgent = 3,
}

/// 分发结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DispatchResult {
    /// 任务 ID
    pub task_id: String,
    /// Agent ID
    pub agent_id: String,
    /// 执行状态
    pub status: DispatchStatus,
    /// 执行结果 (如果完成)
    pub output: Option<AgentOutput>,
    /// 错误信息 (如果失败)
    pub error: Option<String>,
    /// 执行时间 (毫秒)
    pub duration_ms: u64,
}

impl DispatchResult {
    /// 创建成功结果
    pub fn success(
        task_id: impl Into<String>,
        agent_id: impl Into<String>,
        output: AgentOutput,
        duration_ms: u64,
    ) -> Self {
        Self {
            task_id: task_id.into(),
            agent_id: agent_id.into(),
            status: DispatchStatus::Completed,
            output: Some(output),
            error: None,
            duration_ms,
        }
    }

    /// 创建失败结果
    pub fn failure(
        task_id: impl Into<String>,
        agent_id: impl Into<String>,
        error: impl Into<String>,
        duration_ms: u64,
    ) -> Self {
        Self {
            task_id: task_id.into(),
            agent_id: agent_id.into(),
            status: DispatchStatus::Failed,
            output: None,
            error: Some(error.into()),
            duration_ms,
        }
    }

    /// 创建待处理结果
    pub fn pending(task_id: impl Into<String>, agent_id: impl Into<String>) -> Self {
        Self {
            task_id: task_id.into(),
            agent_id: agent_id.into(),
            status: DispatchStatus::Pending,
            output: None,
            error: None,
            duration_ms: 0,
        }
    }
}

/// 分发状态
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum DispatchStatus {
    /// 待处理
    Pending,
    /// 运行中
    Running,
    /// 已完成
    Completed,
    /// 失败
    Failed,
    /// 超时
    Timeout,
    /// 取消
    Cancelled,
}

// ============================================================================
// 聚合策略
// ============================================================================

/// 结果聚合策略
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub enum AggregationStrategy {
    /// 连接所有结果
    Concatenate { separator: String },
    /// 取第一个成功的结果
    FirstSuccess,
    /// 收集所有结果
    #[default]
    CollectAll,
    /// 投票选择
    Vote,
    /// 使用 LLM 总结
    LLMSummarize { prompt_template: String },
    /// 自定义聚合
    Custom(String),
}

/// 聚合结果
pub fn aggregate_outputs(
    outputs: Vec<AgentOutput>,
    strategy: &AggregationStrategy,
) -> AgentResult<AgentOutput> {
    match strategy {
        AggregationStrategy::Concatenate { separator } => {
            let texts: Vec<String> = outputs.iter().map(|o| o.to_text()).collect();
            Ok(AgentOutput::text(texts.join(separator)))
        }
        AggregationStrategy::FirstSuccess => {
            outputs.into_iter().find(|o| !o.is_error()).ok_or_else(|| {
                crate::agent::error::AgentError::CoordinationError(
                    "No successful output".to_string(),
                )
            })
        }
        AggregationStrategy::CollectAll => {
            let texts: Vec<String> = outputs.iter().map(|o| o.to_text()).collect();
            Ok(AgentOutput::json(serde_json::json!({
                "results": texts,
                "count": texts.len(),
            })))
        }
        AggregationStrategy::Vote => {
            // 简单投票：选择最常见的结果
            let mut votes: HashMap<String, usize> = HashMap::new();
            for output in &outputs {
                let text = output.to_text();
                *votes.entry(text).or_insert(0) += 1;
            }
            let winner = votes
                .into_iter()
                .max_by_key(|(_, count)| *count)
                .map(|(text, _)| text)
                .unwrap_or_default();
            Ok(AgentOutput::text(winner))
        }
        AggregationStrategy::LLMSummarize { .. } => {
            // LLM 总结需要外部 LLM 调用，这里只是占位
            let texts: Vec<String> = outputs.iter().map(|o| o.to_text()).collect();
            Ok(AgentOutput::text(texts.join("\n\n---\n\n")))
        }
        AggregationStrategy::Custom(_) => {
            // 自定义聚合需要外部实现
            let texts: Vec<String> = outputs.iter().map(|o| o.to_text()).collect();
            Ok(AgentOutput::text(texts.join("\n")))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_task_creation() {
        let task = Task::new("task-1", "Do something")
            .with_type(TaskType::Analysis)
            .with_priority(TaskPriority::High)
            .for_agent("agent-1")
            .with_timeout(5000);

        assert_eq!(task.id, "task-1");
        assert_eq!(task.task_type, TaskType::Analysis);
        assert_eq!(task.priority, TaskPriority::High);
        assert_eq!(task.target_agent, Some("agent-1".to_string()));
        assert_eq!(task.timeout_ms, Some(5000));
    }

    #[test]
    fn test_dispatch_result() {
        let success =
            DispatchResult::success("task-1", "agent-1", AgentOutput::text("Result"), 100);
        assert_eq!(success.status, DispatchStatus::Completed);
        assert!(success.output.is_some());

        let failure = DispatchResult::failure("task-1", "agent-1", "Error occurred", 50);
        assert_eq!(failure.status, DispatchStatus::Failed);
        assert!(failure.error.is_some());
    }

    #[test]
    fn test_aggregate_concatenate() {
        let outputs = vec![
            AgentOutput::text("Part 1"),
            AgentOutput::text("Part 2"),
            AgentOutput::text("Part 3"),
        ];

        let strategy = AggregationStrategy::Concatenate {
            separator: " | ".to_string(),
        };

        let result = aggregate_outputs(outputs, &strategy).unwrap();
        assert_eq!(result.to_text(), "Part 1 | Part 2 | Part 3");
    }

    #[test]
    fn test_aggregate_first_success() {
        let outputs = vec![
            AgentOutput::error("Error 1"),
            AgentOutput::text("Success"),
            AgentOutput::text("Another success"),
        ];

        let strategy = AggregationStrategy::FirstSuccess;
        let result = aggregate_outputs(outputs, &strategy).unwrap();
        assert_eq!(result.to_text(), "Success");
    }

    #[test]
    fn test_aggregate_vote() {
        let outputs = vec![
            AgentOutput::text("A"),
            AgentOutput::text("B"),
            AgentOutput::text("A"),
            AgentOutput::text("A"),
            AgentOutput::text("B"),
        ];

        let strategy = AggregationStrategy::Vote;
        let result = aggregate_outputs(outputs, &strategy).unwrap();
        assert_eq!(result.to_text(), "A"); // A 有 3 票，B 有 2 票
    }
}
