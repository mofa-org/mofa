//! High-level swarm orchestration entrypoint built on top of the swarm primitives.

use std::collections::HashSet;
use std::sync::Arc;

use chrono::Utc;
use serde_json::json;

use mofa_kernel::agent::types::error::{GlobalError, GlobalResult};

use crate::swarm::{
    AuditEvent, AuditEventKind, CoordinationPattern, HITLMode, SchedulerSummary, SwarmConfig,
    SwarmMetrics, SwarmResult, SwarmScheduler, SwarmSchedulerConfig, SwarmStatus, TaskAnalyzer,
};
use crate::swarm::{SubtaskDAG, SubtaskExecutorFn, SwarmSubtask};

/// Coordinate DAG construction, agent assignment, scheduler execution, and result synthesis.
pub struct SwarmOrchestrator {
    scheduler_config: SwarmSchedulerConfig,
}

impl SwarmOrchestrator {
    /// Create an orchestrator with default scheduler behavior.
    pub fn new() -> Self {
        Self {
            scheduler_config: SwarmSchedulerConfig::default(),
        }
    }

    /// Create an orchestrator with explicit scheduler configuration.
    pub fn with_scheduler_config(scheduler_config: SwarmSchedulerConfig) -> Self {
        Self { scheduler_config }
    }

    /// Build a runnable DAG from the task text using the deterministic offline analyzer.
    pub fn build_dag(&self, config: &SwarmConfig) -> GlobalResult<SubtaskDAG> {
        if config.task.trim().is_empty() {
            return Err(GlobalError::Other(
                "swarm task must not be empty".to_string(),
            ));
        }

        let analysis = TaskAnalyzer::analyze_offline_with_risk(&config.task);
        let mut dag = analysis.dag;
        dag.name = config.name.clone();
        self.apply_hitl_mode(config, &mut dag);
        self.assign_agents(config, &mut dag)?;
        Ok(dag)
    }

    /// Execute an already-constructed DAG and return a full `SwarmResult`.
    pub async fn run_dag(
        &self,
        config: &SwarmConfig,
        mut dag: SubtaskDAG,
        executor: SubtaskExecutorFn,
    ) -> GlobalResult<SwarmResult> {
        self.apply_hitl_mode(config, &mut dag);
        self.assign_agents(config, &mut dag)?;

        let started_at = Utc::now();
        let mut audit_events = vec![
            AuditEvent::new(AuditEventKind::SwarmStarted, "Swarm orchestration started")
                .with_data(json!({"config_id": config.id, "name": config.name})),
            AuditEvent::new(AuditEventKind::TaskDecomposed, "Swarm DAG prepared").with_data(json!({
                "task_count": dag.task_count(),
                "hitl_required": dag.hitl_required_tasks(),
            })),
            AuditEvent::new(
                AuditEventKind::PatternSelected,
                format!("Coordination pattern selected: {}", config.pattern),
            )
            .with_data(json!({"pattern": config.pattern})),
        ];
        self.record_agent_assignments(&dag, &mut audit_events);
        self.record_hitl_requests(&dag, &mut audit_events);

        let summary = self.execute_pattern(config.pattern.clone(), &mut dag, executor).await?;
        let metrics = self.metrics_from_summary(&dag, &summary);
        let status = self.status_from_summary(&summary);
        let output = self.aggregate_output(&summary);

        audit_events.push(
            AuditEvent::new(
                AuditEventKind::SwarmCompleted,
                "Swarm orchestration finished",
            )
            .with_data(json!({
                "status": status,
                "succeeded": summary.succeeded,
                "failed": summary.failed,
                "skipped": summary.skipped,
            })),
        );

        Ok(SwarmResult {
            config_id: config.id.clone(),
            status,
            dag,
            output,
            metrics,
            audit_events,
            started_at,
            completed_at: Some(Utc::now()),
        })
    }

    /// Build a DAG from config and execute it end to end.
    pub async fn run(
        &self,
        config: &SwarmConfig,
        executor: SubtaskExecutorFn,
    ) -> GlobalResult<SwarmResult> {
        let dag = self.build_dag(config)?;
        self.run_dag(config, dag, executor).await
    }

    /// Dispatch execution to the scheduler that matches the requested coordination pattern.
    async fn execute_pattern(
        &self,
        pattern: CoordinationPattern,
        dag: &mut SubtaskDAG,
        executor: SubtaskExecutorFn,
    ) -> GlobalResult<SchedulerSummary> {
        match pattern {
            CoordinationPattern::Sequential => {
                let scheduler = crate::swarm::SequentialScheduler::with_config(
                    self.scheduler_config.clone(),
                );
                scheduler.execute(dag, executor).await
            }
            CoordinationPattern::Parallel => {
                let scheduler =
                    crate::swarm::ParallelScheduler::with_config(self.scheduler_config.clone());
                scheduler.execute(dag, executor).await
            }
            other => Err(GlobalError::Other(format!(
                "coordination pattern `{other}` is not yet implemented for SwarmOrchestrator"
            ))),
        }
    }

    /// Normalize per-task HITL flags against the top-level swarm HITL mode.
    fn apply_hitl_mode(&self, config: &SwarmConfig, dag: &mut SubtaskDAG) {
        let updates: Vec<_> = dag
            .all_tasks()
            .into_iter()
            .map(|(idx, task)| {
                let required = match config.hitl {
                    HITLMode::None => false,
                    HITLMode::Required => true,
                    HITLMode::Optional => task.hitl_required,
                };
                (idx, required)
            })
            .collect();

        for (idx, required) in updates {
            if let Some(task_mut) = dag.get_task_mut(idx) {
                task_mut.hitl_required = required;
            }
        }
    }

    /// Assign each task to the best matching configured agent before execution starts.
    fn assign_agents(&self, config: &SwarmConfig, dag: &mut SubtaskDAG) -> GlobalResult<()> {
        if config.agents.is_empty() {
            return Ok(());
        }

        let assignments: Vec<(petgraph::graph::NodeIndex, String)> = dag
            .all_tasks()
            .into_iter()
            .map(|(idx, task)| self.select_agent_for_task(config, task).map(|agent| (idx, agent)))
            .collect::<GlobalResult<_>>()?;

        for (idx, agent_id) in assignments {
            dag.assign_agent(idx, agent_id);
        }

        Ok(())
    }

    /// Choose an agent whose capabilities satisfy the task, falling back to the first agent.
    fn select_agent_for_task(
        &self,
        config: &SwarmConfig,
        task: &SwarmSubtask,
    ) -> GlobalResult<String> {
        let required: HashSet<&str> = task
            .required_capabilities
            .iter()
            .map(String::as_str)
            .collect();

        if required.is_empty() {
            return config
                .agents
                .first()
                .map(|agent| agent.id.clone())
                .ok_or_else(|| GlobalError::Other("no agents configured for swarm".to_string()));
        }

        config
            .agents
            .iter()
            .find(|agent| {
                required
                    .iter()
                    .all(|cap| agent.capabilities.iter().any(|agent_cap| agent_cap == cap))
            })
            .or_else(|| config.agents.first())
            .map(|agent| agent.id.clone())
            .ok_or_else(|| GlobalError::Other("no agents configured for swarm".to_string()))
    }

    /// Emit audit events for the final task-to-agent assignment plan.
    fn record_agent_assignments(&self, dag: &SubtaskDAG, audit_events: &mut Vec<AuditEvent>) {
        for (_, task) in dag.all_tasks() {
            if let Some(agent) = &task.assigned_agent {
                audit_events.push(
                    AuditEvent::new(
                        AuditEventKind::AgentAssigned,
                        format!("Assigned task `{}` to `{agent}`", task.id),
                    )
                    .with_data(json!({"task_id": task.id, "agent_id": agent})),
                );
            }
        }
    }

    /// Emit audit events for tasks that require human review before execution.
    fn record_hitl_requests(&self, dag: &SubtaskDAG, audit_events: &mut Vec<AuditEvent>) {
        for task_id in dag.hitl_required_tasks() {
            audit_events.push(
                AuditEvent::new(
                    AuditEventKind::HITLRequested,
                    format!("Task `{task_id}` requires human approval"),
                )
                .with_data(json!({"task_id": task_id})),
            );
        }
    }

    /// Translate scheduler-level execution counts into the stable swarm metrics snapshot.
    fn metrics_from_summary(&self, dag: &SubtaskDAG, summary: &SchedulerSummary) -> SwarmMetrics {
        let mut metrics = SwarmMetrics::default();
        for _ in 0..summary.succeeded {
            metrics.record_task_completed();
        }
        for _ in 0..summary.failed {
            metrics.record_task_failed();
        }
        metrics.hitl_interventions = dag.hitl_required_tasks().len();
        metrics.set_duration_ms(summary.total_wall_time.as_millis() as u64);

        for (_, task) in dag.all_tasks() {
            if let Some(agent) = &task.assigned_agent {
                metrics.record_agent_tokens(agent.clone(), 0);
            }
        }

        metrics
    }

    /// Collapse scheduler outcome counts into one top-level swarm status.
    fn status_from_summary(&self, summary: &SchedulerSummary) -> SwarmStatus {
        if summary.failed > 0 {
            SwarmStatus::Failed(format!(
                "{} task(s) failed during swarm execution",
                summary.failed
            ))
        } else {
            SwarmStatus::Completed
        }
    }

    /// Join successful task outputs into one top-level swarm output payload.
    fn aggregate_output(&self, summary: &SchedulerSummary) -> Option<String> {
        let outputs = summary.successful_outputs();
        if outputs.is_empty() {
            None
        } else {
            Some(outputs.join("\n"))
        }
    }
}

impl Default for SwarmOrchestrator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    use futures::future::BoxFuture;

    use crate::swarm::{AgentSpec, RiskLevel};

    fn test_config(pattern: CoordinationPattern) -> SwarmConfig {
        SwarmConfig {
            id: "cfg-test".into(),
            name: "incident-response".into(),
            description: String::new(),
            task: "fetch logs then summarize findings".into(),
            agents: vec![
                AgentSpec {
                    id: "researcher".into(),
                    capabilities: vec!["fetch".into(), "search".into(), "list".into()],
                    model: None,
                    cost_per_token: None,
                    max_concurrency: 1,
                },
                AgentSpec {
                    id: "analyst".into(),
                    capabilities: vec!["summarize".into(), "analyze".into(), "reporting".into()],
                    model: None,
                    cost_per_token: None,
                    max_concurrency: 1,
                },
            ],
            pattern,
            sla: Default::default(),
            hitl: HITLMode::Optional,
            metadata: Default::default(),
        }
    }

    #[tokio::test]
    async fn orchestrator_runs_a_config_end_to_end() {
        let orchestrator = SwarmOrchestrator::new();
        let config = test_config(CoordinationPattern::Sequential);
        let executor = Arc::new(|_idx, task: SwarmSubtask| -> BoxFuture<'static, _> {
            Box::pin(async move { Ok(format!("{} handled", task.id)) })
        });

        let result = orchestrator.run(&config, executor).await.unwrap();

        assert_eq!(result.status, SwarmStatus::Completed);
        assert_eq!(result.metrics.tasks_completed, result.dag.completed_count());
        assert!(result.output.as_deref().unwrap_or_default().contains("handled"));
        assert!(result
            .audit_events
            .iter()
            .any(|event| event.kind == AuditEventKind::PatternSelected));
    }

    #[tokio::test]
    async fn orchestrator_assigns_agents_and_respects_required_hitl() {
        let orchestrator = SwarmOrchestrator::new();
        let mut config = test_config(CoordinationPattern::Sequential);
        config.hitl = HITLMode::Required;

        let dag = orchestrator.build_dag(&config).unwrap();

        assert!(dag.all_tasks().iter().all(|(_, task)| task.hitl_required));
        assert!(dag
            .all_tasks()
            .iter()
            .all(|(_, task)| task.assigned_agent.is_some()));
    }

    #[tokio::test]
    async fn orchestrator_reports_unsupported_patterns() {
        let orchestrator = SwarmOrchestrator::new();
        let config = test_config(CoordinationPattern::Debate);
        let executor = Arc::new(|_idx, task: SwarmSubtask| -> BoxFuture<'static, _> {
            Box::pin(async move {
                tokio::time::sleep(Duration::from_millis(1)).await;
                Ok(task.id)
            })
        });

        let err = orchestrator.run(&config, executor).await.unwrap_err();
        assert!(err.to_string().contains("not yet implemented"));
    }

    #[test]
    fn orchestrator_build_dag_uses_offline_swarm_analysis() {
        let orchestrator = SwarmOrchestrator::new();
        let config = test_config(CoordinationPattern::Sequential);

        let dag = orchestrator.build_dag(&config).unwrap();

        assert_eq!(dag.name, "incident-response");
        assert!(dag.task_count() >= 2);
        assert!(dag
            .all_tasks()
            .iter()
            .any(|(_, task)| matches!(&task.risk_level, RiskLevel::Low | RiskLevel::Medium | RiskLevel::High | RiskLevel::Critical)));
    }
}
