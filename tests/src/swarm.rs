//! Swarm/orchestrator testing helpers with visual artifact rendering.

use std::collections::HashMap;

use mofa_foundation::swarm::{
    AuditEvent, AuditEventKind, CoordinationPattern, RiskLevel, SchedulerSummary, SubtaskDAG,
    SubtaskStatus, SwarmMetrics, SwarmResult, SwarmStatus, TaskOutcome,
};
use serde::{Deserialize, Serialize};

/// Canonical snapshot of one swarm task for testing and review output.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SwarmTaskRecord {
    pub id: String,
    pub description: String,
    pub status: SubtaskStatus,
    pub assigned_agent: Option<String>,
    pub output: Option<String>,
    pub dependencies: Vec<String>,
    pub dependents: Vec<String>,
    pub required_capabilities: Vec<String>,
    pub risk_level: RiskLevel,
    pub hitl_required: bool,
}

/// Visualizable artifact for one orchestrated swarm run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SwarmRunArtifact {
    pub name: String,
    pub pattern: CoordinationPattern,
    pub swarm_status: Option<SwarmStatus>,
    pub total_tasks: usize,
    pub succeeded: usize,
    pub failed: usize,
    pub skipped: usize,
    pub total_wall_time_ms: u64,
    pub success_rate: f64,
    pub output: Option<String>,
    pub metrics: Option<SwarmMetricsRecord>,
    pub audit_events: Vec<SwarmAuditRecord>,
    pub tasks: Vec<SwarmTaskRecord>,
    pub execution: Vec<SwarmExecutionRecord>,
}

/// One scheduler result entry normalized for rendering/assertion.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SwarmExecutionRecord {
    pub task_id: String,
    pub node_index: usize,
    pub outcome: String,
    pub detail: Option<String>,
    pub wall_time_ms: u64,
    pub attempt: u32,
}

/// Normalized metrics snapshot for swarm testing artifacts.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SwarmMetricsRecord {
    pub total_tokens: u64,
    pub duration_ms: u64,
    pub tasks_completed: usize,
    pub tasks_failed: usize,
    pub hitl_interventions: usize,
    pub reassignments: usize,
    pub agent_tokens: HashMap<String, u64>,
}

/// Normalized audit event for assertion and visual review.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SwarmAuditRecord {
    pub kind: AuditEventKind,
    pub description: String,
    pub timestamp_ms: i64,
    pub data: serde_json::Value,
}

impl SwarmRunArtifact {
    /// Build an artifact from a scheduler summary and the final DAG state.
    pub fn from_scheduler_run(dag: &SubtaskDAG, summary: &SchedulerSummary) -> Self {
        let tasks = snapshot_tasks(dag);
        let execution = snapshot_execution(summary);

        Self {
            name: dag.name.clone(),
            pattern: summary.pattern.clone(),
            swarm_status: None,
            total_tasks: summary.total_tasks,
            succeeded: summary.succeeded,
            failed: summary.failed,
            skipped: summary.skipped,
            total_wall_time_ms: summary.total_wall_time.as_millis() as u64,
            success_rate: summary.success_rate(),
            output: None,
            metrics: None,
            audit_events: Vec::new(),
            tasks,
            execution,
        }
    }

    /// Build an artifact from a full swarm result, including metrics and audit events.
    pub fn from_swarm_result(
        result: &SwarmResult,
        pattern: CoordinationPattern,
        summary: Option<&SchedulerSummary>,
    ) -> Self {
        let tasks = snapshot_tasks(&result.dag);
        let execution = summary.map(snapshot_execution).unwrap_or_default();
        let total_tasks = result.dag.task_count();
        let succeeded = summary.map(|it| it.succeeded).unwrap_or_else(|| {
            result
                .dag
                .all_tasks()
                .iter()
                .filter(|(_, task)| task.status == SubtaskStatus::Completed)
                .count()
        });
        let failed = summary.map(|it| it.failed).unwrap_or(result.metrics.tasks_failed);
        let skipped = summary.map(|it| it.skipped).unwrap_or_else(|| {
            result
                .dag
                .all_tasks()
                .iter()
                .filter(|(_, task)| task.status == SubtaskStatus::Skipped)
                .count()
        });
        let total_wall_time_ms = summary
            .map(|it| it.total_wall_time.as_millis() as u64)
            .unwrap_or(result.metrics.duration_ms);
        let success_rate = if total_tasks == 0 {
            1.0
        } else {
            succeeded as f64 / total_tasks as f64
        };

        Self {
            name: result.dag.name.clone(),
            pattern,
            swarm_status: Some(result.status.clone()),
            total_tasks,
            succeeded,
            failed,
            skipped,
            total_wall_time_ms,
            success_rate,
            output: result.output.clone(),
            metrics: Some(SwarmMetricsRecord::from(&result.metrics)),
            audit_events: result.audit_events.iter().map(SwarmAuditRecord::from).collect(),
            tasks,
            execution,
        }
    }

    /// Render the artifact as pretty JSON for CI/artifacts.
    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).expect("swarm artifact serialization should not fail")
    }

    /// Render the artifact as markdown with a Mermaid dependency graph.
    pub fn to_markdown(&self) -> String {
        let mut out = String::new();
        let tasks_by_agent = self.tasks_by_agent();

        out.push_str(&format!("# Swarm Test Artifact: {}\n\n", self.name));
        out.push_str(&format!("- Pattern: `{}`\n", self.pattern_label()));
        out.push_str(&format!("- Total tasks: `{}`\n", self.total_tasks));
        out.push_str(&format!("- Succeeded: `{}`\n", self.succeeded));
        out.push_str(&format!("- Failed: `{}`\n", self.failed));
        out.push_str(&format!("- Skipped: `{}`\n", self.skipped));
        out.push_str(&format!("- Success rate: `{:.1}%`\n", self.success_rate * 100.0));
        if let Some(status) = &self.swarm_status {
            out.push_str(&format!("- Swarm status: `{}`\n", swarm_status_label(status)));
        }
        out.push_str(&format!(
            "- Total wall time: `{} ms`\n\n",
            self.total_wall_time_ms
        ));
        if let Some(output) = &self.output {
            out.push_str(&format!("- Final output: `{}`\n\n", compact(output)));
        }

        out.push_str("## Dependency Graph\n\n```mermaid\ngraph TD\n");
        for task in &self.tasks {
            if task.dependencies.is_empty() {
                out.push_str(&format!("    {}[\"{}\"]\n", sanitize_node_id(&task.id), task.id));
            } else {
                for dep in &task.dependencies {
                    out.push_str(&format!(
                        "    {}[\"{}\"] --> {}[\"{}\"]\n",
                        sanitize_node_id(dep),
                        dep,
                        sanitize_node_id(&task.id),
                        task.id
                    ));
                }
            }
        }
        out.push_str("```\n\n");

        out.push_str("## Tasks\n\n");
        out.push_str("| Task | Status | Agent | Depends On | Capabilities | HITL |\n");
        out.push_str("| --- | --- | --- | --- | --- | --- |\n");
        for task in &self.tasks {
            out.push_str(&format!(
                "| {} | {} | {} | {} | {} | {} |\n",
                task.id,
                status_label(&task.status),
                task.assigned_agent.as_deref().unwrap_or("-"),
                join_or_dash(&task.dependencies),
                join_or_dash(&task.required_capabilities),
                if task.hitl_required { "required" } else { "no" }
            ));
            if let Some(output) = &task.output {
                out.push_str(&format!("| output | `{}` |  |  |  |  |\n", compact(output)));
            }
        }

        out.push_str("\n## Execution Trace\n\n");
        out.push_str("| Order | Task | Outcome | Detail | Duration |\n");
        out.push_str("| --- | --- | --- | --- | --- |\n");
        for (order, record) in self.execution.iter().enumerate() {
            out.push_str(&format!(
                "| {} | {} | {} | {} | {} ms |\n",
                order + 1,
                record.task_id,
                record.outcome,
                record
                    .detail
                    .as_deref()
                    .map(compact)
                    .unwrap_or_else(|| "-".to_string()),
                record.wall_time_ms
            ));
        }

        if !tasks_by_agent.is_empty() {
            let mut agent_names: Vec<_> = tasks_by_agent.keys().cloned().collect();
            agent_names.sort();

            out.push_str("\n## Agent Collaboration View\n\n");
            out.push_str("| Agent | Tasks | Terminal States |\n");
            out.push_str("| --- | --- | --- |\n");

            for agent in agent_names {
                if let Some(tasks) = tasks_by_agent.get(&agent) {
                    let task_ids: Vec<String> = tasks.iter().map(|task| task.id.clone()).collect();
                    let statuses: Vec<String> = tasks
                        .iter()
                        .map(|task| format!("{}:{}", task.id, status_label(&task.status)))
                        .collect();
                    out.push_str(&format!(
                        "| {} | {} | {} |\n",
                        agent,
                        join_or_dash(&task_ids),
                        join_or_dash(&statuses)
                    ));
                }
            }
        }

        if let Some(metrics) = &self.metrics {
            let mut agents: Vec<_> = metrics.agent_tokens.iter().collect();
            agents.sort_by(|a, b| a.0.cmp(b.0));

            out.push_str("\n## Metrics\n\n");
            out.push_str("| Total Tokens | Duration | Tasks Completed | Tasks Failed | HITL | Reassignments |\n");
            out.push_str("| --- | --- | --- | --- | --- | --- |\n");
            out.push_str(&format!(
                "| {} | {} ms | {} | {} | {} | {} |\n",
                metrics.total_tokens,
                metrics.duration_ms,
                metrics.tasks_completed,
                metrics.tasks_failed,
                metrics.hitl_interventions,
                metrics.reassignments
            ));

            if !agents.is_empty() {
                out.push_str("\n| Agent | Tokens |\n");
                out.push_str("| --- | --- |\n");
                for (agent, tokens) in agents {
                    out.push_str(&format!("| {} | {} |\n", agent, tokens));
                }
            }
        }

        if !self.audit_events.is_empty() {
            out.push_str("\n## Audit Trail\n\n");
            out.push_str("| Event | Description | Data |\n");
            out.push_str("| --- | --- | --- |\n");
            for event in &self.audit_events {
                // Keep event payloads compact so the markdown stays PR-comment friendly.
                out.push_str(&format!(
                    "| {} | {} | `{}` |\n",
                    audit_kind_label(&event.kind),
                    compact(&event.description),
                    compact(&event.data.to_string())
                ));
            }
        }

        out
    }

    /// Assert a specific task exists and has the expected status.
    pub fn assert_task_status(
        &self,
        task_id: &str,
        expected: SubtaskStatus,
    ) -> Result<(), String> {
        let actual = self
            .task(task_id)
            .ok_or_else(|| format!("task `{task_id}` not found"))?;

        if actual.status == expected {
            Ok(())
        } else {
            Err(format!(
                "task `{task_id}` expected status `{}` but was `{}`",
                status_label(&expected),
                status_label(&actual.status)
            ))
        }
    }

    /// Assert that all tasks completed successfully.
    pub fn assert_all_completed(&self) -> Result<(), String> {
        let incomplete: Vec<String> = self
            .tasks
            .iter()
            .filter(|task| task.status != SubtaskStatus::Completed)
            .map(|task| format!("{}:{}", task.id, status_label(&task.status)))
            .collect();

        if incomplete.is_empty() {
            Ok(())
        } else {
            Err(format!(
                "expected all tasks completed, found non-completed tasks: {}",
                incomplete.join(", ")
            ))
        }
    }

    /// Assert aggregate scheduler counts.
    pub fn assert_counts(
        &self,
        succeeded: usize,
        failed: usize,
        skipped: usize,
    ) -> Result<(), String> {
        if self.succeeded == succeeded && self.failed == failed && self.skipped == skipped {
            Ok(())
        } else {
            Err(format!(
                "expected counts success/fail/skip = {}/{}/{}, got {}/{}/{}",
                succeeded, failed, skipped, self.succeeded, self.failed, self.skipped
            ))
        }
    }

    /// Assert a dependency edge exists in the decomposed task graph.
    pub fn assert_dependency(&self, task_id: &str, dependency_id: &str) -> Result<(), String> {
        let task = self
            .task(task_id)
            .ok_or_else(|| format!("task `{task_id}` not found"))?;

        if task.dependencies.iter().any(|dep| dep == dependency_id) {
            Ok(())
        } else {
            Err(format!(
                "task `{task_id}` does not depend on `{dependency_id}`"
            ))
        }
    }

    /// Assert a task output contains the provided substring.
    pub fn assert_output_contains(&self, task_id: &str, needle: &str) -> Result<(), String> {
        let task = self
            .task(task_id)
            .ok_or_else(|| format!("task `{task_id}` not found"))?;
        let output = task
            .output
            .as_deref()
            .ok_or_else(|| format!("task `{task_id}` has no output"))?;

        if output.contains(needle) {
            Ok(())
        } else {
            Err(format!(
                "task `{task_id}` output did not contain `{needle}`"
            ))
        }
    }

    /// Assert a task failed and its failure message contains the provided substring.
    pub fn assert_task_failed_contains(&self, task_id: &str, needle: &str) -> Result<(), String> {
        let task = self
            .task(task_id)
            .ok_or_else(|| format!("task `{task_id}` not found"))?;

        match &task.status {
            SubtaskStatus::Failed(reason) if reason.contains(needle) => Ok(()),
            SubtaskStatus::Failed(reason) => Err(format!(
                "task `{task_id}` failed, but `{reason}` did not contain `{needle}`"
            )),
            status => Err(format!(
                "task `{task_id}` expected failed status but was `{}`",
                status_label(status)
            )),
        }
    }

    /// Group tasks by assigned agent for multi-agent collaboration assertions.
    pub fn tasks_by_agent(&self) -> HashMap<String, Vec<&SwarmTaskRecord>> {
        let mut by_agent = HashMap::new();
        for task in &self.tasks {
            if let Some(agent) = &task.assigned_agent {
                by_agent
                    .entry(agent.clone())
                    .or_insert_with(Vec::new)
                    .push(task);
            }
        }
        by_agent
    }

    /// Assert that a given agent was assigned the expected task.
    pub fn assert_agent_has_task(&self, agent_id: &str, task_id: &str) -> Result<(), String> {
        let by_agent = self.tasks_by_agent();
        let tasks = by_agent
            .get(agent_id)
            .ok_or_else(|| format!("agent `{agent_id}` not found in artifact"))?;

        if tasks.iter().any(|task| task.id == task_id) {
            Ok(())
        } else {
            Err(format!(
                "agent `{agent_id}` was not assigned task `{task_id}`"
            ))
        }
    }

    /// Assert swarm-level metrics match expected HITL and reassignment counts.
    pub fn assert_metrics(
        &self,
        hitl_interventions: usize,
        reassignments: usize,
    ) -> Result<(), String> {
        let metrics = self
            .metrics
            .as_ref()
            .ok_or_else(|| "artifact has no metrics snapshot".to_string())?;

        if metrics.hitl_interventions == hitl_interventions
            && metrics.reassignments == reassignments
        {
            Ok(())
        } else {
            Err(format!(
                "expected metrics hitl/reassignments = {}/{}, got {}/{}",
                hitl_interventions,
                reassignments,
                metrics.hitl_interventions,
                metrics.reassignments
            ))
        }
    }

    /// Assert at least one audit event of the given kind exists.
    pub fn assert_audit_event_kind(&self, kind: AuditEventKind) -> Result<(), String> {
        if self.audit_events.iter().any(|event| event.kind == kind) {
            Ok(())
        } else {
            Err(format!(
                "artifact does not contain audit event `{}`",
                audit_kind_label(&kind)
            ))
        }
    }

    fn task(&self, task_id: &str) -> Option<&SwarmTaskRecord> {
        self.tasks.iter().find(|task| task.id == task_id)
    }

    fn pattern_label(&self) -> &'static str {
        match self.pattern {
            CoordinationPattern::Sequential => "sequential",
            CoordinationPattern::Parallel => "parallel",
            CoordinationPattern::Debate => "debate",
            CoordinationPattern::Consensus => "consensus",
            CoordinationPattern::MapReduce => "map_reduce",
            CoordinationPattern::Supervision => "supervision",
            CoordinationPattern::Routing => "routing",
        }
    }
}

impl From<&SwarmMetrics> for SwarmMetricsRecord {
    fn from(value: &SwarmMetrics) -> Self {
        Self {
            total_tokens: value.total_tokens,
            duration_ms: value.duration_ms,
            tasks_completed: value.tasks_completed,
            tasks_failed: value.tasks_failed,
            hitl_interventions: value.hitl_interventions,
            reassignments: value.reassignments,
            agent_tokens: value.agent_tokens.clone(),
        }
    }
}

impl From<&AuditEvent> for SwarmAuditRecord {
    fn from(value: &AuditEvent) -> Self {
        Self {
            kind: value.kind.clone(),
            description: value.description.clone(),
            timestamp_ms: value.timestamp.timestamp_millis(),
            data: value.data.clone(),
        }
    }
}

fn snapshot_tasks(dag: &SubtaskDAG) -> Vec<SwarmTaskRecord> {
    // Prefer topological order so the rendered artifact reads like the intended
    // orchestration plan rather than raw graph insertion order.
    let ordered = dag
        .topological_order()
        .unwrap_or_else(|_| dag.all_tasks().into_iter().map(|(idx, _)| idx).collect());

    let mut tasks = Vec::new();
    for idx in ordered {
        if let Some(task) = dag.get_task(idx) {
            let dependencies = dag
                .dependencies_of(idx)
                .into_iter()
                .filter_map(|dep| dag.get_task(dep).map(|task| task.id.clone()))
                .collect();
            let dependents = dag
                .dependents_of(idx)
                .into_iter()
                .filter_map(|dep| dag.get_task(dep).map(|task| task.id.clone()))
                .collect();

            tasks.push(SwarmTaskRecord {
                id: task.id.clone(),
                description: task.description.clone(),
                status: task.status.clone(),
                assigned_agent: task.assigned_agent.clone(),
                output: task.output.clone(),
                dependencies,
                dependents,
                required_capabilities: task.required_capabilities.clone(),
                risk_level: task.risk_level.clone(),
                hitl_required: task.hitl_required,
            });
        }
    }
    tasks
}

fn snapshot_execution(summary: &SchedulerSummary) -> Vec<SwarmExecutionRecord> {
    summary
        .results
        .iter()
        .map(|result| {
            // Normalize scheduler outcomes to stable strings
            let (outcome, detail) = match &result.outcome {
                TaskOutcome::Success(output) => ("success".to_string(), Some(output.clone())),
                TaskOutcome::Failure(error) => ("failure".to_string(), Some(error.clone())),
                TaskOutcome::Skipped(reason) => ("skipped".to_string(), Some(reason.clone())),
            };

            SwarmExecutionRecord {
                task_id: result.task_id.clone(),
                node_index: result.node_index,
                outcome,
                detail,
                wall_time_ms: result.wall_time.as_millis() as u64,
                attempt: result.attempt,
            }
        })
        .collect()
}

fn join_or_dash(values: &[String]) -> String {
    if values.is_empty() {
        "-".to_string()
    } else {
        values.join(", ")
    }
}

fn sanitize_node_id(id: &str) -> String {
    id.chars()
        .map(|ch| if ch.is_ascii_alphanumeric() { ch } else { '_' })
        .collect()
}

fn compact(value: &str) -> String {
    let trimmed = value.replace('\n', " ");
    if trimmed.len() > 60 {
        format!("{}...", &trimmed[..57])
    } else {
        trimmed
    }
}

fn status_label(status: &SubtaskStatus) -> String {
    match status {
        SubtaskStatus::Pending => "pending".to_string(),
        SubtaskStatus::Ready => "ready".to_string(),
        SubtaskStatus::Running => "running".to_string(),
        SubtaskStatus::Completed => "completed".to_string(),
        SubtaskStatus::Failed(reason) => format!("failed ({reason})"),
        SubtaskStatus::Skipped => "skipped".to_string(),
    }
}

fn swarm_status_label(status: &SwarmStatus) -> String {
    match status {
        SwarmStatus::Running => "running".to_string(),
        SwarmStatus::Completed => "completed".to_string(),
        SwarmStatus::Failed(reason) => format!("failed ({reason})"),
        SwarmStatus::Cancelled => "cancelled".to_string(),
        SwarmStatus::Escalated => "escalated".to_string(),
        _ => "unknown".to_string(),
    }
}

fn audit_kind_label(kind: &AuditEventKind) -> &'static str {
    match kind {
        AuditEventKind::SwarmStarted => "swarm_started",
        AuditEventKind::TaskDecomposed => "task_decomposed",
        AuditEventKind::AgentAssigned => "agent_assigned",
        AuditEventKind::PatternSelected => "pattern_selected",
        AuditEventKind::SubtaskStarted => "subtask_started",
        AuditEventKind::SubtaskCompleted => "subtask_completed",
        AuditEventKind::SubtaskFailed => "subtask_failed",
        AuditEventKind::HITLRequested => "hitl_requested",
        AuditEventKind::HITLDecision => "hitl_decision",
        AuditEventKind::SLAWarning => "sla_warning",
        AuditEventKind::SLABreach => "sla_breach",
        AuditEventKind::AgentReassigned => "agent_reassigned",
        AuditEventKind::SwarmCompleted => "swarm_completed",
        _ => "unknown_event",
    }
}
