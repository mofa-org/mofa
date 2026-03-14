use mofa_kernel::agent::types::error::{GlobalError, GlobalResult};
mod scheduler;
#[cfg(test)]
mod tests;
use mofa_kernel::message::{AgentMessage, TaskRequest, TaskStatus};
use mofa_kernel::{AgentBus, CommunicationMode};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::time::{Duration, timeout};

const PIPELINE_STAGE_TIMEOUT_SECS: u64 = 5;

/// 协同策略枚举
/// Enumeration of coordination strategies
#[derive(Debug, Clone)]
pub enum CoordinationStrategy {
    MasterSlave, // 主从模式
    // Master-Slave mode
    PeerToPeer, // 对等协作模式
    // Peer-to-Peer collaboration mode
    Pipeline, // 流水线模式（任务串行传递）
              // Pipeline mode (serial task passing)
}

/// 协同器核心结构体
/// Core structure of the Agent Coordinator
pub struct AgentCoordinator {
    bus: Arc<AgentBus>,
    strategy: CoordinationStrategy,
    // 维护协同拓扑：角色→智能体ID列表
    // Maintain coordination topology: Role -> Agent ID list
    role_mapping: Arc<RwLock<HashMap<String, Vec<String>>>>,
    // 维护任务状态：任务ID→执行智能体ID+状态（列表）
    // Maintain task status: Task ID -> list of (Executor Agent ID, Status)
    task_tracker: Arc<RwLock<HashMap<String, Vec<(String, TaskStatus)>>>>,
    // 优先级调度器
    // Priority scheduler
    scheduler: scheduler::PriorityScheduler,
}

impl AgentCoordinator {
    fn extract_pipeline_task(task_msg: &AgentMessage) -> GlobalResult<(String, String)> {
        match task_msg {
            AgentMessage::TaskRequest { task_id, content } => {
                Ok((task_id.clone(), content.clone()))
            }
            _ => Err(GlobalError::Other(
                "Pipeline coordination only supports TaskRequest messages".to_string(),
            )),
        }
    }

    /// 创建协同器
    /// Create a new coordinator
    pub async fn new(bus: Arc<AgentBus>, strategy: CoordinationStrategy) -> Self {
        let scheduler = scheduler::PriorityScheduler::new(bus.clone()).await;
        Self {
            bus,
            strategy,
            role_mapping: Arc::new(RwLock::new(HashMap::new())),
            task_tracker: Arc::new(RwLock::new(HashMap::new())),
            scheduler,
        }
    }

    /// 注册智能体角色（如 "master"/"worker"/"data_provider"）
    /// Register agent roles (e.g., "master"/"worker"/"data_provider")
    pub async fn register_role(&self, agent_id: &str, role: &str) -> GlobalResult<()> {
        let mut role_map = self.role_mapping.write().await;
        role_map
            .entry(role.to_string())
            .or_default()
            .push(agent_id.to_string());
        Ok(())
    }

    /// 执行协同任务：根据策略自动分配任务给对应角色的智能体
    /// Execute coordination task: auto-assign tasks based on strategy and roles
    pub async fn coordinate_task(&self, task_msg: &AgentMessage) -> GlobalResult<()> {
        match &self.strategy {
            CoordinationStrategy::MasterSlave => self.master_slave_coordinate(task_msg).await,
            CoordinationStrategy::Pipeline => self.pipeline_coordinate(task_msg).await,
            CoordinationStrategy::PeerToPeer => self.peer_to_peer_coordinate(task_msg).await,
        }
    }

    /// 对外暴露的接口：提交带优先级的任务
    /// Public interface: Submit tasks with priority levels
    pub async fn submit_priority_task(&self, task: TaskRequest) -> GlobalResult<()> {
        self.scheduler.submit_task(task).await
    }

    /// 主从模式协同逻辑（核心示例）
    /// Master-Slave coordination logic (Core example)
    async fn master_slave_coordinate(&self, task_msg: &AgentMessage) -> GlobalResult<()> {
        let role_map = self.role_mapping.read().await;
        // 1. 获取主智能体（负责任务分配）
        // 1. Get the Master agent (responsible for task distribution)
        let masters = role_map
            .get("master")
            .ok_or_else(|| GlobalError::Other("No master agent registered".to_string()))?;
        let master_id = &masters[0];
        // 2. 获取所有 worker 智能体（负责执行任务）
        // 2. Get all worker agents (responsible for task execution)
        let workers = role_map
            .get("worker")
            .ok_or_else(|| GlobalError::Other("No worker agents registered".to_string()))?;

        // 3. 主智能体广播任务给所有 worker
        // 3. Master agent broadcasts the task to all workers
        self.bus
            .send_message(master_id, CommunicationMode::Broadcast, task_msg)
            .await
            .map_err(|e| GlobalError::Other(e.to_string()))?;

        // 4. 跟踪任务状态
        // 4. Track task status for all workers
        if let AgentMessage::TaskRequest { task_id, .. } = task_msg {
            let mut tracker = self.task_tracker.write().await;
            let entries = tracker.entry(task_id.clone()).or_default();
            for worker_id in workers {
                entries.push((worker_id.clone(), TaskStatus::Pending));
            }
        }
        Ok(())
    }

    /// 流水线模式协同逻辑（任务串行传递）
    /// Pipeline coordination logic (Serial task transmission)
    async fn pipeline_coordinate(&self, task_msg: &AgentMessage) -> GlobalResult<()> {
        let (root_task_id, initial_content) = Self::extract_pipeline_task(task_msg)?;
        let role_map = self.role_mapping.read().await;
        // 按流水线阶段顺序获取角色（如 "stage1_extract" → "stage2_process" → "stage3_output"）
        // Get roles by pipeline sequence (e.g., "stage1_extract" -> "stage2_process" -> "stage3_output")
        let stages = vec!["stage1", "stage2", "stage3"];
        // Carry the previous stage's output forward as the next stage's input.
        let mut last_output: Option<String> = Some(initial_content);

        for stage in stages {
            let agents = role_map
                .get(stage)
                .ok_or_else(|| GlobalError::Other(format!("No agent for stage {}", stage)))?;
            let agent_id = &agents[0];
            // 传递上一阶段输出作为当前阶段输入
            // Pass the output of the previous stage as current stage input
            let current_msg = AgentMessage::TaskRequest {
                // Keep one root task id across the full pipeline for traceability.
                task_id: root_task_id.clone(),
                content: last_output.clone().ok_or_else(|| {
                    GlobalError::Other(format!("Pipeline stage {} has no upstream output", stage))
                })?,
            };
            let response_bus = self.bus.clone();
            let response_agent_id = agent_id.to_string();
            let response_handle = tokio::spawn(async move {
                response_bus
                    .receive_message(
                        "coordinator",
                        CommunicationMode::PointToPoint(response_agent_id),
                    )
                    .await
            });
            tokio::task::yield_now().await;
            // 点对点发送给当前阶段智能体
            // Send point-to-point to the agent of the current stage
            self.bus
                .send_message(
                    "coordinator",
                    CommunicationMode::PointToPoint(agent_id.to_string()),
                    &current_msg,
                )
                .await
                .map_err(|e| GlobalError::Other(e.to_string()))?;
            // Wait on the coordinator side reply channel with a bounded timeout.
            let stage_response = timeout(
                Duration::from_secs(PIPELINE_STAGE_TIMEOUT_SECS),
                response_handle,
            )
            .await
            .map_err(|_| {
                GlobalError::Other(format!(
                    "Timed out waiting for pipeline stage {} ({})",
                    stage, agent_id
                ))
            })?
            .map_err(|e| {
                GlobalError::Other(format!("Pipeline stage {} join failed: {}", stage, e))
            })?
            .map_err(|e| GlobalError::Other(e.to_string()))?;

            match stage_response {
                Some(AgentMessage::TaskResponse {
                    task_id,
                    result,
                    status,
                }) => {
                    // A stage must answer for the same root task the pipeline started with.
                    if task_id != root_task_id {
                        return Err(GlobalError::Other(format!(
                            "Pipeline stage {} returned mismatched task id: expected {}, got {}",
                            stage, root_task_id, task_id
                        )));
                    }
                    if status != TaskStatus::Success {
                        return Err(GlobalError::Other(format!(
                            "Pipeline stage {} ({}) failed for task {}",
                            stage, agent_id, root_task_id
                        )));
                    }
                    last_output = Some(result);
                }
                Some(other) => {
                    return Err(GlobalError::Other(format!(
                        "Pipeline stage {} ({}) returned unexpected message: {:?}",
                        stage, agent_id, other
                    )));
                }
                None => {
                    return Err(GlobalError::Other(format!(
                        "Pipeline stage {} ({}) closed without a response",
                        stage, agent_id
                    )));
                }
            }
        }
        Ok(())
    }

    /// Peer-to-peer coordination logic
    async fn peer_to_peer_coordinate(&self, task_msg: &AgentMessage) -> GlobalResult<()> {
        let role_map = self.role_mapping.read().await;

        let peers = role_map
            .get("peer")
            .ok_or_else(|| GlobalError::Other("No peer agents registered".to_string()))?;

        // Send point to point to all peers to avoid broadcast leakage
        for peer_id in peers {
            self.bus
                .send_message(
                    "coordinator",
                    CommunicationMode::PointToPoint(peer_id.clone()),
                    task_msg,
                )
                .await
                .map_err(|e| GlobalError::Other(e.to_string()))?;
        }

        // Track task status
        if let AgentMessage::TaskRequest { task_id, .. } = task_msg {
            let mut tracker = self.task_tracker.write().await;
            let entries = tracker.entry(task_id.clone()).or_default();
            for peer_id in peers {
                entries.push((peer_id.clone(), TaskStatus::Pending));
            }
        }
        Ok(())
    }
}
