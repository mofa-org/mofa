//! 任务协调器 - 阶段3: 调度分配，调用执行Agent

use super::types::*;
use crate::secretary::agent_router::{
    AgentInfo, AgentProvider, AgentRouter, CapabilityRouter, RoutingContext, RoutingDecision,
};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// 分配策略
#[derive(Debug, Clone)]
pub enum DispatchStrategy {
    /// 轮询分配
    RoundRobin,
    /// 最低负载优先
    LeastLoaded,
    /// 能力匹配优先
    CapabilityFirst,
    /// 性能评分优先
    PerformanceFirst,
    /// 综合评分
    Composite {
        capability_weight: f32,
        load_weight: f32,
        performance_weight: f32,
    },
    /// 动态路由
    Dynamic,
}

/// 任务分配结果
#[derive(Debug, Clone)]
pub struct DispatchResult {
    /// 子任务ID
    pub subtask_id: String,
    /// 分配的Agent ID
    pub agent_id: String,
    /// 匹配分数
    pub match_score: f32,
    /// 分配时间
    pub dispatched_at: u64,
    /// 路由决策
    pub routing_decision: Option<RoutingDecision>,
}

/// 任务协调器
pub struct TaskCoordinator {
    /// 可用的执行Agent列表
    executors: Arc<RwLock<HashMap<String, AgentInfo>>>,
    /// 分配策略
    strategy: DispatchStrategy,
    /// 任务分配记录
    dispatch_records: Arc<RwLock<Vec<DispatchResult>>>,
    /// 轮询索引
    round_robin_index: Arc<RwLock<usize>>,
    /// 消息发送回调
    message_sender: Option<Arc<dyn Fn(SecretaryMessage, &str) -> anyhow::Result<()> + Send + Sync>>,
    /// 动态Agent提供者
    agent_provider: Option<Arc<dyn AgentProvider>>,
    /// Agent路由器
    agent_router: Option<Arc<dyn AgentRouter>>,
}

impl TaskCoordinator {
    /// 创建新的任务协调器
    pub fn new(strategy: DispatchStrategy) -> Self {
        Self {
            executors: Arc::new(RwLock::new(HashMap::new())),
            strategy,
            dispatch_records: Arc::new(RwLock::new(Vec::new())),
            round_robin_index: Arc::new(RwLock::new(0)),
            message_sender: None,
            agent_provider: None,
            agent_router: None,
        }
    }

    /// 创建使用动态路由的协调器
    pub fn with_dynamic_routing() -> Self {
        Self::new(DispatchStrategy::Dynamic)
    }

    /// 设置消息发送回调
    pub fn with_message_sender<F>(mut self, sender: F) -> Self
    where
        F: Fn(SecretaryMessage, &str) -> anyhow::Result<()> + Send + Sync + 'static,
    {
        self.message_sender = Some(Arc::new(sender));
        self
    }

    /// 设置动态Agent提供者
    pub fn with_agent_provider(mut self, provider: Arc<dyn AgentProvider>) -> Self {
        self.agent_provider = Some(provider);
        self
    }

    /// 设置Agent路由器
    pub fn with_agent_router(mut self, router: Arc<dyn AgentRouter>) -> Self {
        self.agent_router = Some(router);
        self
    }

    /// 设置Agent提供者（非消费式）
    pub fn set_agent_provider(&mut self, provider: Arc<dyn AgentProvider>) {
        self.agent_provider = Some(provider);
    }

    /// 设置Agent路由器（非消费式）
    pub fn set_agent_router(&mut self, router: Arc<dyn AgentRouter>) {
        self.agent_router = Some(router);
    }

    /// 获取Agent提供者
    pub fn agent_provider(&self) -> Option<&Arc<dyn AgentProvider>> {
        self.agent_provider.as_ref()
    }

    /// 获取Agent路由器
    pub fn agent_router(&self) -> Option<&Arc<dyn AgentRouter>> {
        self.agent_router.as_ref()
    }

    /// 注册执行Agent
    pub async fn register_executor(&self, executor: AgentInfo) {
        let mut executors = self.executors.write().await;
        tracing::info!(
            "Registered executor: {} with capabilities: {:?}",
            executor.id,
            executor.capabilities
        );
        executors.insert(executor.id.clone(), executor);
    }

    /// 注销执行Agent
    pub async fn unregister_executor(&self, agent_id: &str) {
        let mut executors = self.executors.write().await;
        executors.remove(agent_id);
        tracing::info!("Unregistered executor: {}", agent_id);
    }

    /// 更新执行Agent状态
    pub async fn update_executor_status(&self, agent_id: &str, load: u32, available: bool) {
        let mut executors = self.executors.write().await;
        if let Some(executor) = executors.get_mut(agent_id) {
            executor.current_load = load;
            executor.available = available;
        }
    }

    /// 获取所有可用执行Agent
    pub async fn list_available_executors(&self) -> Vec<AgentInfo> {
        let executors = self.executors.read().await;
        executors
            .values()
            .filter(|e| e.available)
            .cloned()
            .collect()
    }

    /// 为子任务分配执行Agent
    pub async fn dispatch_subtask(&self, subtask: &Subtask) -> anyhow::Result<DispatchResult> {
        if matches!(self.strategy, DispatchStrategy::Dynamic) {
            return self.dispatch_subtask_dynamic(subtask, None).await;
        }

        let executors = self.executors.read().await;
        let available: Vec<_> = executors.values().filter(|e| e.available).collect();

        if available.is_empty() {
            return Err(anyhow::anyhow!("No available executors"));
        }

        let (selected, score): (&AgentInfo, f32) = match &self.strategy {
            DispatchStrategy::RoundRobin => {
                let mut idx = self.round_robin_index.write().await;
                let selected = available[*idx % available.len()];
                *idx = (*idx + 1) % available.len();
                (selected, 1.0)
            }
            DispatchStrategy::LeastLoaded => {
                let selected = available
                    .iter()
                    .copied()
                    .min_by_key(|e| e.current_load)
                    .ok_or_else(|| anyhow::anyhow!("No available executors"))?;
                let score = 1.0 - (selected.current_load as f32 / 100.0);
                (selected, score)
            }
            DispatchStrategy::CapabilityFirst => {
                self.select_by_capability(&available, &subtask.required_capabilities)?
            }
            DispatchStrategy::PerformanceFirst => {
                let selected = available
                    .iter()
                    .copied()
                    .max_by(|a, b| {
                        a.performance_score
                            .partial_cmp(&b.performance_score)
                            .unwrap_or(std::cmp::Ordering::Equal)
                    })
                    .ok_or_else(|| anyhow::anyhow!("No available executors"))?;
                (selected, selected.performance_score)
            }
            DispatchStrategy::Composite {
                capability_weight,
                load_weight,
                performance_weight,
            } => self.select_composite(
                &available,
                &subtask.required_capabilities,
                *capability_weight,
                *load_weight,
                *performance_weight,
            )?,
            DispatchStrategy::Dynamic => unreachable!(),
        };

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let result = DispatchResult {
            subtask_id: subtask.id.clone(),
            agent_id: selected.id.clone(),
            match_score: score,
            dispatched_at: now,
            routing_decision: None,
        };

        {
            let mut records = self.dispatch_records.write().await;
            records.push(result.clone());
        }

        tracing::info!(
            "Dispatched subtask {} to agent {} with score {}",
            subtask.id,
            selected.id,
            score
        );

        Ok(result)
    }

    /// 使用动态路由分配子任务
    pub async fn dispatch_subtask_dynamic(
        &self,
        subtask: &Subtask,
        context: Option<&RoutingContext>,
    ) -> anyhow::Result<DispatchResult> {
        let available_agents = self.get_available_agents().await?;

        if available_agents.is_empty() {
            return Err(anyhow::anyhow!("No available agents"));
        }

        let routing_context = if let Some(ctx) = context {
            ctx.clone()
        } else {
            RoutingContext::new(subtask.clone(), "")
        };

        let decision = if let Some(ref router) = self.agent_router {
            router.route(&routing_context, &available_agents).await?
        } else {
            let default_router = CapabilityRouter::new();
            default_router
                .route(&routing_context, &available_agents)
                .await?
        };

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let result = DispatchResult {
            subtask_id: subtask.id.clone(),
            agent_id: decision.agent_id.clone(),
            match_score: decision.confidence,
            dispatched_at: now,
            routing_decision: Some(decision.clone()),
        };

        {
            let mut records = self.dispatch_records.write().await;
            records.push(result.clone());
        }

        tracing::info!(
            "Dynamically dispatched subtask {} to agent {} with confidence {} (reason: {})",
            subtask.id,
            decision.agent_id,
            decision.confidence,
            decision.reason
        );

        Ok(result)
    }

    /// 获取可用Agent列表
    pub async fn get_available_agents(&self) -> anyhow::Result<Vec<AgentInfo>> {
        if let Some(ref provider) = self.agent_provider {
            return Ok(provider.list_agents().await);
        }

        let executors = self.executors.read().await;
        let agents: Vec<AgentInfo> = executors
            .values()
            .filter(|e| e.available)
            .cloned()
            .collect();

        Ok(agents)
    }

    fn select_by_capability<'a>(
        &self,
        available: &[&'a AgentInfo],
        required_capabilities: &[String],
    ) -> anyhow::Result<(&'a AgentInfo, f32)> {
        let mut best: Option<(&AgentInfo, f32)> = None;

        for executor in available {
            let match_count = required_capabilities
                .iter()
                .filter(|cap| executor.capabilities.contains(cap))
                .count();

            let score = if required_capabilities.is_empty() {
                1.0
            } else {
                match_count as f32 / required_capabilities.len() as f32
            };

            if best.is_none() || score > best.unwrap().1 {
                best = Some((executor, score));
            }
        }

        best.ok_or_else(|| anyhow::anyhow!("No matching executor found"))
    }

    fn select_composite<'a>(
        &self,
        available: &[&'a AgentInfo],
        required_capabilities: &[String],
        capability_weight: f32,
        load_weight: f32,
        performance_weight: f32,
    ) -> anyhow::Result<(&'a AgentInfo, f32)> {
        let mut best: Option<(&AgentInfo, f32)> = None;

        for executor in available {
            let capability_score = if required_capabilities.is_empty() {
                1.0
            } else {
                let match_count = required_capabilities
                    .iter()
                    .filter(|cap| executor.capabilities.contains(cap))
                    .count();
                match_count as f32 / required_capabilities.len() as f32
            };

            let load_score = 1.0 - (executor.current_load as f32 / 100.0);
            let performance_score = executor.performance_score;

            let total_score = capability_score * capability_weight
                + load_score * load_weight
                + performance_score * performance_weight;

            if best.is_none() || total_score > best.unwrap().1 {
                best = Some((executor, total_score));
            }
        }

        best.ok_or_else(|| anyhow::anyhow!("No matching executor found"))
    }

    /// 为需求的所有子任务分配Agent
    pub async fn dispatch_requirement(
        &self,
        requirement: &ProjectRequirement,
        context: HashMap<String, String>,
    ) -> anyhow::Result<Vec<DispatchResult>> {
        let mut results = Vec::new();
        let mut pending_subtasks: Vec<&Subtask> = requirement.subtasks.iter().collect();
        pending_subtasks.sort_by_key(|s| s.order);

        for subtask in pending_subtasks {
            let result = self.dispatch_subtask(subtask).await?;

            if let Some(ref sender) = self.message_sender {
                let message = SecretaryMessage::AssignTask {
                    task_id: subtask.id.clone(),
                    subtask: subtask.clone(),
                    context: context.clone(),
                };
                sender(message, &result.agent_id)?;
            }

            results.push(result);
        }

        Ok(results)
    }

    /// 取消任务
    pub async fn cancel_task(&self, task_id: &str, reason: &str) -> anyhow::Result<()> {
        let records = self.dispatch_records.read().await;
        let record = records
            .iter()
            .find(|r| r.subtask_id == task_id)
            .ok_or_else(|| anyhow::anyhow!("Task not found: {}", task_id))?;

        if let Some(ref sender) = self.message_sender {
            let message = SecretaryMessage::CancelTask {
                task_id: task_id.to_string(),
                reason: reason.to_string(),
            };
            sender(message, &record.agent_id)?;
        }

        tracing::info!("Cancelled task {} on agent {}", task_id, record.agent_id);
        Ok(())
    }

    /// 获取分配记录
    pub async fn get_dispatch_records(&self) -> Vec<DispatchResult> {
        let records = self.dispatch_records.read().await;
        records.clone()
    }

    /// 获取Agent的分配统计
    pub async fn get_agent_statistics(&self) -> HashMap<String, usize> {
        let records = self.dispatch_records.read().await;
        let mut stats: HashMap<String, usize> = HashMap::new();

        for record in records.iter() {
            *stats.entry(record.agent_id.clone()).or_insert(0) += 1;
        }

        stats
    }
}

impl Default for TaskCoordinator {
    fn default() -> Self {
        Self::new(DispatchStrategy::CapabilityFirst)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_agent(id: &str, name: &str, capability: &str) -> AgentInfo {
        let mut agent = AgentInfo::new(id, name);
        agent.capabilities = vec![capability.to_string()];
        agent.current_load = 0;
        agent.available = true;
        agent.performance_score = 0.8;
        agent
    }

    #[tokio::test]
    async fn test_register_executor() {
        let coordinator = TaskCoordinator::new(DispatchStrategy::RoundRobin);

        coordinator
            .register_executor(make_agent("agent_1", "Test Agent", "backend"))
            .await;

        let executors = coordinator.list_available_executors().await;
        assert_eq!(executors.len(), 1);
    }

    #[tokio::test]
    async fn test_dispatch_by_capability() {
        let coordinator = TaskCoordinator::new(DispatchStrategy::CapabilityFirst);

        coordinator
            .register_executor(make_agent("frontend_agent", "Frontend Agent", "frontend"))
            .await;

        coordinator
            .register_executor(make_agent("backend_agent", "Backend Agent", "backend"))
            .await;

        let subtask = Subtask {
            id: "task_1".to_string(),
            description: "Build API".to_string(),
            required_capabilities: vec!["backend".to_string()],
            order: 1,
            depends_on: Vec::new(),
        };

        let result = coordinator.dispatch_subtask(&subtask).await.unwrap();
        assert_eq!(result.agent_id, "backend_agent");
    }
}
