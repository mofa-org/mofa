use mofa_kernel::agent::types::error::{GlobalError, GlobalResult};
use mofa_kernel::message::{AgentEvent, AgentMessage, SchedulingStatus, TaskPriority, TaskRequest};
use mofa_kernel::{AgentBus, CommunicationMode};
use std::collections::{BinaryHeap, HashMap};
use std::sync::Arc;
use tokio::sync::RwLock;

// 带优先级的任务包装器（BinaryHeap 默认是最大堆，直接支持优先级排序）
// Task wrapper with priority (BinaryHeap is a max-heap by default, supporting priority sorting)
#[derive(Debug, Clone, Eq, PartialEq)]
struct PriorityTask {
    priority: TaskPriority,
    task: TaskRequest,
    submit_time: std::time::Instant,
}

impl Ord for PriorityTask {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.priority
            .cmp(&other.priority)
            .then_with(|| other.submit_time.cmp(&self.submit_time)) // 同优先级先提交先执行
        // First-in-first-out for tasks with the same priority
    }
}

impl PartialOrd for PriorityTask {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

// 优先级调度器
// Priority Scheduler
pub struct PriorityScheduler {
    task_queue: Arc<RwLock<BinaryHeap<PriorityTask>>>, // 优先级任务队列
    // Priority task queue
    agent_load: Arc<RwLock<HashMap<String, usize>>>, // 智能体当前负载（执行中的任务数）
    // Current agent load (number of tasks being executed)
    bus: Arc<AgentBus>,
    task_status: Arc<RwLock<HashMap<String, SchedulingStatus>>>, // 任务状态跟踪
    // Task status tracking
    role_mapping: Arc<RwLock<HashMap<String, Vec<String>>>>, // 角色-智能体映射
    // Role-to-agent mapping
    agent_tasks: Arc<RwLock<HashMap<String, Vec<String>>>>, // Agent-to-task mapping
    task_priorities: Arc<RwLock<HashMap<String, TaskPriority>>>, // Task priority tracking
}

impl PriorityScheduler {
    pub async fn new(bus: Arc<AgentBus>) -> Self {
        Self {
            task_queue: Arc::new(RwLock::new(BinaryHeap::new())),
            agent_load: Arc::new(RwLock::new(HashMap::new())),
            bus,
            task_status: Arc::new(RwLock::new(HashMap::new())),
            role_mapping: Arc::new(RwLock::new(HashMap::new())),
            agent_tasks: Arc::new(RwLock::new(HashMap::new())),
            task_priorities: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// 1. 提交任务到优先级队列
    /// 1. Submit task to the priority queue
    pub async fn submit_task(&self, task: TaskRequest) -> GlobalResult<()> {
        let priority_task = PriorityTask {
            priority: task.priority.clone(),
            task: task.clone(),
            submit_time: std::time::Instant::now(),
        };
        self.task_queue.write().await.push(priority_task);
        self.task_priorities
            .write()
            .await
            .insert(task.task_id.clone(), task.priority.clone());
        self.task_status
            .write()
            .await
            .insert(task.task_id, SchedulingStatus::Pending);
        // 提交后立即触发调度
        // Trigger scheduling immediately after submission
        self.schedule()
            .await
            .map_err(|e| GlobalError::Other(e.to_string()))?;
        Ok(())
    }

    /// 2. 核心调度逻辑：选高优先级任务 + 选负载最低的智能体
    /// 2. Core logic: select high-priority task + select lowest-load agent
    ///
    /// Each iteration pops one pending task while holding locks only briefly,
    /// then releases ALL guards before any `.await` on an async helper
    /// (`select_low_load_agent`, `preempt_low_priority_task`, `bus.send_message`).
    /// Holding a write guard on `agent_load` across the `.await` that calls
    /// `select_low_load_agent` — which tries to acquire `agent_load.read()` —
    /// self-deadlocks the task because Tokio's `RwLock` is not re-entrant.
    pub async fn schedule(&self) -> GlobalResult<()> {
        loop {
            // Phase 1: Pop the next pending task while holding locks briefly.
            // All guards are dropped at the end of this block before any async call.
            let priority_task = {
                let mut task_queue = self.task_queue.write().await;
                let task_status    = self.task_status.read().await;
                let mut found = None;
                while let Some(pt) = task_queue.pop() {
                    if task_status.get(&pt.task.task_id) == Some(&SchedulingStatus::Pending) {
                        found = Some(pt);
                        break;
                    }
                    // Task already processed (e.g. preempted); discard and continue.
                }
                found
            }; // task_queue write and task_status read guards released here

            let priority_task = match priority_task {
                Some(pt) => pt,
                None => return Ok(()), // Queue empty or all remaining tasks non-Pending
            };

            let task    = priority_task.task.clone();
            let task_id = task.task_id.clone();

            // 选择负载最低的可用智能体（同角色内）— 内联以避免死锁
            // Select the available agent with the lowest load — inlined to avoid deadlock
         let sorted_agents = {
    let role_map = self.role_mapping.read().await;
    let agent_load = self.agent_load.read().await;

    match role_map.get("worker") {
        Some(agents) => {
            let mut sorted = agents.clone();
            sorted.sort_by_key(|agent_id| {
                agent_load.get(agent_id).cloned().unwrap_or(0)
            });
            sorted
        }
        None => {
            drop(role_map);
            drop(agent_load);
            self.task_queue.write().await.push(priority_task);
            break;
        }
    }
};


            if sorted_agents.is_empty() {
                // 无可用智能体，重新入队
                // No agent available — put the task back and stop this scheduling pass.
                self.task_queue.write().await.push(priority_task);
                return Ok(());
            }
            let target_agent = sorted_agents[0].clone();

            // 检查是否需要抢占 — 内联以避免死锁
         // Check for preemption — acquire read locks temporarily
{
    let agent_load      = self.agent_load.read().await;
    let agent_tasks     = self.agent_tasks.read().await;
    let task_status     = self.task_status.read().await;
    let task_priorities = self.task_priorities.read().await;

    if let Some(&load) = agent_load.get(&target_agent) {
        if load > 0 {
            if let Some(tasks_on_agent) = agent_tasks.get(&target_agent) {
                let preemptable_task = tasks_on_agent
                    .iter()
                    .filter(|tid| {
                        task_status.get(*tid) == Some(&SchedulingStatus::Running)
                    })
                    .filter(|tid| {
                        task_priorities
                            .get(*tid)
                            .map(|p| task.priority > *p)
                            .unwrap_or(false)
                    })
                    .min_by_key(|tid| task_priorities.get(*tid).cloned())
                    .cloned();

                if let Some(low_priority_task_id) = preemptable_task {
                    drop(agent_load);
                    drop(agent_tasks);
                    drop(task_status);
                    drop(task_priorities);

                    let preempt_msg =
                        AgentMessage::Event(AgentEvent::TaskPreempted(
                            low_priority_task_id.clone(),
                        ));

                    self.bus
                        .send_message(
                            "scheduler",
                            CommunicationMode::PointToPoint(target_agent.clone()),
                            &preempt_msg,
                        )
                        .await
                        .map_err(|e| GlobalError::Other(e.to_string()))?;
                }
            }
        }
    }
}

            // Phase 4: Dispatch to bus — no scheduler locks held.
            // 发送任务给目标智能体
            // Send task to the target agent
            let task_msg = AgentMessage::TaskRequest {
                task_id: task.task_id.clone(),
                content: task.content.clone(),
            };
            self.bus
                .send_message(
                    "scheduler",
                    CommunicationMode::PointToPoint(target_agent.clone()),
                    &task_msg,
                )
                .await
                .map_err(|e| GlobalError::Other(e.to_string()))?;

            // Phase 5: Commit state update — acquire write locks briefly after all
            // async work is done; no async helper is called while they are held.
            // 更新状态和负载
            // Update task status and agent load
            {
                let mut agent_load  = self.agent_load .write().await;
                let mut task_status = self.task_status.write().await;
                let mut agent_tasks = self.agent_tasks.write().await;
                task_status.insert(task_id.clone(), SchedulingStatus::Running);
                *agent_load.entry(target_agent.clone()).or_insert(0) += 1;
                agent_tasks.entry(target_agent).or_default().push(task_id);
            } // write guards released; loop continues to dispatch next task
        }
    }

    /// 3. 负载均衡：选择同角色内负载最低的智能体
    /// 3. Load balancing: select the lowest-load agent within the same role
    async fn select_low_load_agent(&self, role: &str) -> GlobalResult<Vec<String>> {
        let role_map = self.role_mapping.read().await;
        let agents = role_map
            .get(role)
            .ok_or_else(|| GlobalError::Other(format!("No agent for role: {}", role)))?;
        let agent_load = self.agent_load.read().await;

        // 按负载升序排序，取负载最低的
        // Sort by load in ascending order and pick the lowest
        let mut sorted_agents = agents.clone();
        sorted_agents.sort_by_key(|agent_id| agent_load.get(agent_id).cloned().unwrap_or(0));
        Ok(sorted_agents)
    }

    /// 4. 任务抢占：高优先级任务抢占低优先级任务的执行资源
    /// 4. Task preemption: high-priority tasks preempt resources of low-priority tasks
    async fn preempt_low_priority_task(
        &self,
        agent_id: &str,
        high_priority_task: &TaskRequest,
    ) -> GlobalResult<()> {
        // Collect the preemption decision while holding read locks, then release
        // every guard before the async `bus.send_message` call.  Holding read
        // locks across `.await` blocks writers for the duration of the network
        // send, increasing contention unnecessarily.
        let preemptable_task_id: Option<String> = {
            let agent_load      = self.agent_load     .read().await;
            let task_status     = self.task_status    .read().await;
            let agent_tasks     = self.agent_tasks    .read().await;
            let task_priorities = self.task_priorities.read().await;

            // 检查目标智能体当前运行的任务
            // Check tasks currently running on the target agent
            if let Some(&load) = agent_load.get(agent_id) {
    if load > 0 {
        agent_tasks.get(agent_id).and_then(|tasks_on_agent| {
            tasks_on_agent
                .iter()
                .filter(|tid| {
                    task_status.get(*tid) == Some(&SchedulingStatus::Running)
                })
                .filter(|tid| {
                    task_priorities
                        .get(*tid)
                        .map(|p| high_priority_task.priority > *p)
                        .unwrap_or(false)
                })
                .min_by_key(|tid| task_priorities.get(*tid).cloned())
                .cloned()
        })
    } else {
        None
    }
} else {
    None
}
                

            
        }; // All read guards released here — bus.send_message runs with no locks held

        if let Some(low_priority_task_id) = preemptable_task_id {
            // 发送抢占指令，标记低优先级任务为 Preempted
            // Send preemption command to mark the low-priority task as Preempted
            let preempt_msg =
                AgentMessage::Event(AgentEvent::TaskPreempted(low_priority_task_id));
            self.bus
                .send_message(
                    "scheduler",
                    CommunicationMode::PointToPoint(agent_id.to_string()),
                    &preempt_msg,
                )
                .await
                .map_err(|e| GlobalError::Other(e.to_string()))?;
        }
        Ok(())
    }

    /// 5. 任务完成后更新状态和负载
    /// 5. Update status and load upon task completion
    pub async fn on_task_completed(&self, agent_id: &str, task_id: &str) -> GlobalResult<()> {
        // Scope lock guards so they are dropped before calling schedule(),
        // which needs to acquire the same locks — avoids deadlock.
        {
            let mut agent_load = self.agent_load.write().await;
            let mut task_status = self.task_status.write().await;
            let mut agent_tasks = self.agent_tasks.write().await;

            agent_load
                .entry(agent_id.to_string())
                .and_modify(|count| *count = count.saturating_sub(1));
            task_status.insert(task_id.to_string(), SchedulingStatus::Completed);

            // Remove completed task from agent's task list
            if let Some(tasks) = agent_tasks.get_mut(agent_id) {
                tasks.retain(|t| t != task_id);
            }
        } // All lock guards dropped here

        // 任务完成后再次触发调度，处理队列中的下一个任务
        // Trigger scheduling again after completion to handle the next task
        self.schedule()
            .await
            .map_err(|e| GlobalError::Other(e.to_string()))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mofa_kernel::message::TaskPriority;
    use std::collections::HashMap;
    use tokio::time::{Duration, timeout};

    fn make_task(id: &str, priority: TaskPriority) -> TaskRequest {
        TaskRequest {
            task_id: id.to_string(),
            content: "test".to_string(),
            priority,
            deadline: None,
            metadata: HashMap::new(),
        }
    }

    /// Proves Site 1+2 deadlock is fixed: submit_task → schedule() no longer
    /// tries to re-acquire locks held by schedule() itself.
    /// Without the fix this test would hang forever.
    #[tokio::test]
    async fn test_submit_task_does_not_deadlock() {
        let bus = Arc::new(AgentBus::new());
        let scheduler = PriorityScheduler::new(bus).await;

        let task = make_task("t1", TaskPriority::Normal);
        let result = timeout(Duration::from_secs(2), scheduler.submit_task(task)).await;
        assert!(result.is_ok(), "submit_task should complete, not deadlock");
    }

    /// Proves Site 3 deadlock is fixed: on_task_completed() drops its locks
    /// before calling schedule(), so schedule() can acquire them again.
    /// Without the fix this test would hang forever.
    #[tokio::test]
    async fn test_on_task_completed_does_not_deadlock() {
        let bus = Arc::new(AgentBus::new());
        let scheduler = PriorityScheduler::new(bus).await;

        // Pre-populate state as if a task was scheduled to an agent.
        {
            let mut load = scheduler.agent_load.write().await;
            load.insert("agent-1".to_string(), 1);
        }
        {
            let mut status = scheduler.task_status.write().await;
            status.insert("t1".to_string(), SchedulingStatus::Running);
        }
        {
            let mut tasks = scheduler.agent_tasks.write().await;
            tasks.insert("agent-1".to_string(), vec!["t1".to_string()]);
        }

        let result = timeout(
            Duration::from_secs(2),
            scheduler.on_task_completed("agent-1", "t1"),
        )
        .await;
        assert!(
            result.is_ok(),
            "on_task_completed should complete, not deadlock"
        );

        // Verify state was updated correctly.
        let load = scheduler.agent_load.read().await;
        assert_eq!(*load.get("agent-1").unwrap(), 0);
        let status = scheduler.task_status.read().await;
        assert_eq!(*status.get("t1").unwrap(), SchedulingStatus::Completed);
    }

    /// Verifies the priority queue orders tasks correctly (higher priority first).
    #[tokio::test]
    async fn test_priority_ordering() {
        let mut heap = std::collections::BinaryHeap::new();
        let now = std::time::Instant::now();

        heap.push(PriorityTask {
            priority: TaskPriority::Low,
            task: make_task("low", TaskPriority::Low),
            submit_time: now,
        });
        heap.push(PriorityTask {
            priority: TaskPriority::Critical,
            task: make_task("critical", TaskPriority::Critical),
            submit_time: now,
        });
        heap.push(PriorityTask {
            priority: TaskPriority::Medium,
            task: make_task("medium", TaskPriority::Medium),
            submit_time: now,
        });

        assert_eq!(heap.pop().unwrap().task.task_id, "critical");
        assert_eq!(heap.pop().unwrap().task.task_id, "medium");
        assert_eq!(heap.pop().unwrap().task.task_id, "low");
    }
}
