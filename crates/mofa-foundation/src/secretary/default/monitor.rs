//! 任务监控器 - 阶段4: 监控反馈，推送关键决策给人类
//! Task Monitor - Phase 4: Monitor feedback, push critical decisions to humans

use mofa_kernel::agent::types::error::{GlobalError, GlobalResult};
use super::types::*;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{RwLock, mpsc};

/// 监控事件
/// Monitoring events
#[derive(Debug, Clone)]
pub enum MonitorEvent {
    /// 任务开始
    /// Task started
    TaskStarted { task_id: String, agent_id: String },
    /// 任务进度更新
    /// Task progress update
    TaskProgress {
        task_id: String,
        progress: u32,
        message: Option<String>,
    },
    /// 任务完成
    /// Task completed
    TaskCompleted {
        task_id: String,
        result: ExecutionResult,
    },
    /// 任务失败
    /// Task failed
    TaskFailed { task_id: String, error: String },
    /// 需要决策
    /// Decision required
    DecisionRequired { decision: CriticalDecision },
}

/// 任务快照
/// Task snapshot
#[derive(Debug, Clone)]
pub struct TaskSnapshot {
    /// 任务ID
    /// Task ID
    pub task_id: String,
    /// 执行Agent ID
    /// Execution Agent ID
    pub agent_id: String,
    /// 当前状态
    /// Current status
    pub status: TaskExecutionStatus,
    /// 进度（0-100）
    /// Progress (0-100)
    pub progress: u32,
    /// 最后更新时间
    /// Last updated time
    pub last_updated: u64,
    /// 执行结果（如果已完成）
    /// Execution result (if completed)
    pub result: Option<ExecutionResult>,
}

/// 任务监控器
/// Task monitor
pub struct TaskMonitor {
    /// 任务快照
    /// Task snapshots
    snapshots: Arc<RwLock<HashMap<String, TaskSnapshot>>>,
    /// 待处理的决策
    /// Pending decisions
    pending_decisions: Arc<RwLock<HashMap<String, CriticalDecision>>>,
    /// 决策响应通道
    /// Decision response channels
    decision_responses: Arc<RwLock<HashMap<String, mpsc::Sender<HumanResponse>>>>,
    /// 事件发送器
    /// Event transmitter
    event_tx: Option<mpsc::Sender<MonitorEvent>>,
}

impl TaskMonitor {
    /// 创建新的任务监控器
    /// Create a new task monitor
    pub fn new() -> Self {
        Self {
            snapshots: Arc::new(RwLock::new(HashMap::new())),
            pending_decisions: Arc::new(RwLock::new(HashMap::new())),
            decision_responses: Arc::new(RwLock::new(HashMap::new())),
            event_tx: None,
        }
    }

    /// 设置事件发送器
    /// Set event transmitter
    pub fn with_event_sender(mut self, tx: mpsc::Sender<MonitorEvent>) -> Self {
        self.event_tx = Some(tx);
        self
    }

    /// 开始监控任务
    /// Start monitoring task
    pub async fn start_monitoring(&self, task_id: &str, agent_id: &str) {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let snapshot = TaskSnapshot {
            task_id: task_id.to_string(),
            agent_id: agent_id.to_string(),
            status: TaskExecutionStatus::Received,
            progress: 0,
            last_updated: now,
            result: None,
        };

        {
            let mut snapshots = self.snapshots.write().await;
            snapshots.insert(task_id.to_string(), snapshot);
        }

        self.emit_event(MonitorEvent::TaskStarted {
            task_id: task_id.to_string(),
            agent_id: agent_id.to_string(),
        })
        .await;

        tracing::info!("Started monitoring task {} on agent {}", task_id, agent_id);
    }

    /// 更新任务状态
    /// Update task status
    pub async fn update_task_status(
        &self,
        task_id: &str,
        status: TaskExecutionStatus,
        progress: u32,
        message: Option<String>,
    ) {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        {
            let mut snapshots = self.snapshots.write().await;
            if let Some(snapshot) = snapshots.get_mut(task_id) {
                snapshot.status = status.clone();
                snapshot.progress = progress;
                snapshot.last_updated = now;
            }
        }

        self.emit_event(MonitorEvent::TaskProgress {
            task_id: task_id.to_string(),
            progress,
            message,
        })
        .await;
    }

    /// 任务完成
    /// Task completed
    pub async fn complete_task(&self, task_id: &str, result: ExecutionResult) {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        {
            let mut snapshots = self.snapshots.write().await;
            if let Some(snapshot) = snapshots.get_mut(task_id) {
                snapshot.status = TaskExecutionStatus::Completed;
                snapshot.progress = 100;
                snapshot.last_updated = now;
                snapshot.result = Some(result.clone());
            }
        }

        self.emit_event(MonitorEvent::TaskCompleted {
            task_id: task_id.to_string(),
            result,
        })
        .await;

        tracing::info!("Task {} completed", task_id);
    }

    /// 任务失败
    /// Task failed
    pub async fn fail_task(&self, task_id: &str, error: &str) {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        {
            let mut snapshots = self.snapshots.write().await;
            if let Some(snapshot) = snapshots.get_mut(task_id) {
                snapshot.status = TaskExecutionStatus::Failed(error.to_string());
                snapshot.last_updated = now;
            }
        }

        self.emit_event(MonitorEvent::TaskFailed {
            task_id: task_id.to_string(),
            error: error.to_string(),
        })
        .await;

        tracing::warn!("Task {} failed: {}", task_id, error);
    }

    /// 获取任务快照
    /// Get task snapshot
    pub async fn get_task_snapshot(&self, task_id: &str) -> Option<TaskSnapshot> {
        let snapshots = self.snapshots.read().await;
        snapshots.get(task_id).cloned()
    }

    /// 获取所有任务快照
    /// Get all task snapshots
    pub async fn get_all_snapshots(&self) -> Vec<TaskSnapshot> {
        let snapshots = self.snapshots.read().await;
        snapshots.values().cloned().collect()
    }

    /// 创建决策请求
    /// Create decision request
    pub async fn create_decision(
        &self,
        todo_id: &str,
        decision_type: DecisionType,
        description: &str,
        options: Vec<DecisionOption>,
        recommended_option: Option<usize>,
        deadline: Option<u64>,
    ) -> CriticalDecision {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let decision_id = format!("decision_{}_{}", todo_id, now);

        CriticalDecision {
            id: decision_id,
            todo_id: todo_id.to_string(),
            decision_type,
            description: description.to_string(),
            options,
            recommended_option,
            deadline,
            created_at: now,
            human_response: None,
        }
    }

    /// 请求人类决策
    /// Request human decision
    pub async fn request_decision(
        &self,
        decision: CriticalDecision,
    ) -> GlobalResult<HumanResponse> {
        let decision_id = decision.id.clone();
        let (tx, mut rx) = mpsc::channel(1);

        {
            let mut pending = self.pending_decisions.write().await;
            pending.insert(decision_id.clone(), decision.clone());

            let mut responses = self.decision_responses.write().await;
            responses.insert(decision_id.clone(), tx);
        }

        self.emit_event(MonitorEvent::DecisionRequired { decision })
            .await;

        // 等待人类响应
        // Wait for human response
        rx.recv()
            .await
            .ok_or_else(|| GlobalError::Other("Decision channel closed".to_string()))
    }

    /// 提交人类响应
    /// Submit human response
    pub async fn submit_human_response(
        &self,
        decision_id: &str,
        selected_option: usize,
        comment: Option<String>,
    ) -> GlobalResult<()> {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let response = HumanResponse {
            selected_option,
            comment,
            responded_at: now,
        };

        // 更新决策
        // Update decision
        {
            let mut pending = self.pending_decisions.write().await;
            if let Some(decision) = pending.get_mut(decision_id) {
                decision.human_response = Some(response.clone());
            }
        }

        // 发送响应
        // Send response
        {
            let mut responses = self.decision_responses.write().await;
            if let Some(tx) = responses.remove(decision_id) {
                tx.send(response)
                    .await
                    .map_err(|_| GlobalError::Other("Failed to send response".to_string()))?;
            }
        }

        // 清理
        // Cleanup
        {
            let mut pending = self.pending_decisions.write().await;
            pending.remove(decision_id);
        }

        tracing::info!("Human response submitted for decision {}", decision_id);
        Ok(())
    }

    /// 获取待处理的决策
    /// Get pending decisions
    pub async fn get_pending_decisions(&self) -> Vec<CriticalDecision> {
        let pending = self.pending_decisions.read().await;
        pending.values().cloned().collect()
    }

    /// 处理来自执行Agent的消息
    /// Handle messages from execution agents
    pub async fn handle_agent_message(&self, message: SecretaryMessage) -> GlobalResult<()> {
        match message {
            SecretaryMessage::TaskStatusReport {
                task_id,
                status,
                progress,
                message,
            } => {
                self.update_task_status(&task_id, status, progress, message)
                    .await;
            }
            SecretaryMessage::TaskCompleteReport { task_id, result } => {
                self.complete_task(&task_id, result).await;
            }
            SecretaryMessage::RequestDecision { decision, .. } => {
                let mut pending = self.pending_decisions.write().await;
                pending.insert(decision.id.clone(), decision.clone());

                self.emit_event(MonitorEvent::DecisionRequired { decision })
                    .await;
            }
            _ => {}
        }
        Ok(())
    }

    /// 发送监控事件
    /// Emit monitoring event
    async fn emit_event(&self, event: MonitorEvent) {
        if let Some(ref tx) = self.event_tx {
            let _ = tx.send(event).await;
        }
    }

    /// 获取统计信息
    /// Get statistics
    pub async fn get_statistics(&self) -> HashMap<String, usize> {
        let snapshots = self.snapshots.read().await;
        let mut stats = HashMap::new();

        stats.insert("total_tasks".to_string(), snapshots.len());

        let completed = snapshots
            .values()
            .filter(|s| matches!(s.status, TaskExecutionStatus::Completed))
            .count();
        stats.insert("completed_tasks".to_string(), completed);

        let in_progress = snapshots
            .values()
            .filter(|s| matches!(s.status, TaskExecutionStatus::Executing))
            .count();
        stats.insert("in_progress_tasks".to_string(), in_progress);

        let pending_decisions = self.pending_decisions.read().await;
        stats.insert("pending_decisions".to_string(), pending_decisions.len());

        stats
    }
}

impl Default for TaskMonitor {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_start_monitoring() {
        let monitor = TaskMonitor::new();
        monitor.start_monitoring("task_1", "agent_1").await;

        let snapshot = monitor.get_task_snapshot("task_1").await.unwrap();
        assert_eq!(snapshot.task_id, "task_1");
        assert_eq!(snapshot.agent_id, "agent_1");
    }

    #[tokio::test]
    async fn test_update_status() {
        let monitor = TaskMonitor::new();
        monitor.start_monitoring("task_1", "agent_1").await;

        monitor
            .update_task_status("task_1", TaskExecutionStatus::Executing, 50, None)
            .await;

        let snapshot = monitor.get_task_snapshot("task_1").await.unwrap();
        assert_eq!(snapshot.progress, 50);
    }

    #[tokio::test]
    async fn test_complete_task() {
        let monitor = TaskMonitor::new();
        monitor.start_monitoring("task_1", "agent_1").await;

        let result = ExecutionResult {
            success: true,
            summary: "Done".to_string(),
            details: HashMap::new(),
            artifacts: vec![],
            execution_time_ms: 1000,
            error: None,
        };

        monitor.complete_task("task_1", result).await;

        let snapshot = monitor.get_task_snapshot("task_1").await.unwrap();
        assert!(matches!(snapshot.status, TaskExecutionStatus::Completed));
        assert_eq!(snapshot.progress, 100);
    }
}
