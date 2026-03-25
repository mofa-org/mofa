//! Integration tests for swarm/orchestrator testing artifacts and assertions.
//!
//! Covers multi-agent task decomposition, orchestration state assertions,
//! fail-fast cascade behavior, and the visual markdown/JSON artifacts exposed
//! by `mofa-testing` for maintainers and CI review.

use std::sync::Arc;
use std::time::Duration;

use chrono::Utc;
use futures::future::BoxFuture;
use mofa_foundation::swarm::{
    AuditEvent, AuditEventKind, FailurePolicy, ParallelScheduler, RiskLevel, SubtaskDAG,
    SubtaskStatus, SwarmMetrics, SwarmResult, SwarmScheduler, SwarmSchedulerConfig, SwarmStatus,
    SwarmSubtask, CoordinationPattern,
};
use mofa_kernel::agent::types::error::GlobalError;
use mofa_testing::SwarmRunArtifact;

fn workflow_dag() -> SubtaskDAG {
    let mut dag = SubtaskDAG::new("support-escalation");
    let classify = dag.add_task(
        SwarmSubtask::new("classify", "Classify inbound request")
            .with_capabilities(vec!["triage".into()]),
    );
    let investigate = dag.add_task(
        SwarmSubtask::new("investigate", "Investigate account state")
            .with_capabilities(vec!["lookup".into()])
            .with_risk_level(RiskLevel::Medium),
    );
    let approve = dag.add_task(
        SwarmSubtask::new("approve", "Approve account change")
            .with_capabilities(vec!["approval".into()])
            .with_risk_level(RiskLevel::High),
    );
    let notify = dag.add_task(
        SwarmSubtask::new("notify", "Notify customer")
            .with_capabilities(vec!["email".into()]),
    );

    dag.add_dependency(classify, investigate).unwrap();
    dag.add_dependency(investigate, approve).unwrap();
    dag.add_dependency(approve, notify).unwrap();

    dag.assign_agent(classify, "router");
    dag.assign_agent(investigate, "analyst");
    dag.assign_agent(approve, "reviewer");
    dag.assign_agent(notify, "notifier");

    dag
}

#[tokio::test]
async fn swarm_artifact_captures_decomposition_and_collaboration() {
    let mut dag = workflow_dag();

    let exec = Arc::new(|_idx, task: SwarmSubtask| -> BoxFuture<'static, _> {
        Box::pin(async move {
            tokio::time::sleep(Duration::from_millis(5)).await;
            Ok(format!("{} completed by {}", task.id, task.assigned_agent.unwrap()))
        })
    });

    let scheduler = ParallelScheduler::new();
    let summary = scheduler.execute(&mut dag, exec).await.unwrap();
    let artifact = SwarmRunArtifact::from_scheduler_run(&dag, &summary);

    artifact.assert_all_completed().unwrap();
    artifact.assert_counts(4, 0, 0).unwrap();
    artifact.assert_dependency("approve", "investigate").unwrap();
    artifact
        .assert_task_status("approve", SubtaskStatus::Completed)
        .unwrap();
    artifact
        .assert_output_contains("notify", "notifier")
        .unwrap();

    let by_agent = artifact.tasks_by_agent();
    assert_eq!(by_agent["reviewer"][0].id, "approve");
    artifact.assert_agent_has_task("reviewer", "approve").unwrap();
    assert!(artifact.tasks.iter().any(|task| task.id == "approve" && task.hitl_required));
    assert!(artifact
        .to_markdown()
        .contains("graph TD"));
    assert!(artifact.to_markdown().contains("approve"));
    assert!(artifact.to_markdown().contains("Agent Collaboration View"));
    assert!(artifact.to_markdown().contains("| reviewer | approve |"));
    assert!(artifact.to_json().contains("\"assigned_agent\": \"reviewer\""));

    let approve_task = artifact
        .tasks
        .iter()
        .find(|task| task.id == "approve")
        .unwrap();
    assert_eq!(approve_task.risk_level, RiskLevel::High);
    let notify_task = artifact.tasks.iter().find(|task| task.id == "notify").unwrap();
    assert_eq!(notify_task.dependencies, vec!["approve".to_string()]);
    let approve = dag.find_by_id("approve").unwrap();
    let notify = dag.find_by_id("notify").unwrap();
    assert_eq!(dag.get_task(approve).unwrap().status, SubtaskStatus::Completed);
    assert_eq!(dag.get_task(notify).unwrap().status, SubtaskStatus::Completed);
}

#[tokio::test]
async fn swarm_artifact_surfaces_failed_orchestration_state() {
    let mut dag = workflow_dag();

    let exec = Arc::new(|_idx, task: SwarmSubtask| -> BoxFuture<'static, _> {
        Box::pin(async move {
            if task.id == "approve" {
                Err(GlobalError::runtime("approval rejected"))
            } else {
                Ok(format!("{} ok", task.id))
            }
        })
    });

    let mut config = SwarmSchedulerConfig::default();
    config.failure_policy = FailurePolicy::FailFastCascade;
    let scheduler = ParallelScheduler::with_config(config);
    let summary = scheduler.execute(&mut dag, exec).await.unwrap();
    let artifact = SwarmRunArtifact::from_scheduler_run(&dag, &summary);

    artifact.assert_counts(2, 1, 1).unwrap();
    artifact.assert_task_failed_contains("approve", "approval rejected").unwrap();
    artifact
        .assert_task_status("notify", SubtaskStatus::Skipped)
        .unwrap();

    let err = artifact.assert_all_completed().unwrap_err();
    assert!(err.contains("approve"));
    assert!(artifact.to_markdown().contains("approval rejected"));
    assert!(artifact.to_json().contains("\"outcome\": \"failure\""));
    let approve = dag.find_by_id("approve").unwrap();
    let notify = dag.find_by_id("notify").unwrap();
    assert!(matches!(
        dag.get_task(approve).unwrap().status,
        SubtaskStatus::Failed(_)
    ));
    assert_eq!(dag.get_task(notify).unwrap().status, SubtaskStatus::Skipped);
}

#[tokio::test]
async fn swarm_artifact_from_swarm_result_captures_metrics_and_audit() {
    let dag = workflow_dag();

    let mut metrics = SwarmMetrics::default();
    metrics.record_task_completed();
    metrics.record_task_completed();
    metrics.record_task_failed();
    metrics.record_hitl_intervention();
    metrics.record_reassignment();
    metrics.record_agent_tokens("reviewer", 120);
    metrics.record_agent_tokens("analyst", 60);
    metrics.set_duration_ms(321);

    let audit_events = vec![
        AuditEvent::new(AuditEventKind::SwarmStarted, "Swarm started")
            .with_data(serde_json::json!({"swarm_id":"support-escalation"})),
        AuditEvent::new(AuditEventKind::AgentReassigned, "Reviewer took over approve")
            .with_data(serde_json::json!({"subtask_id":"approve","new_agent":"reviewer"})),
        AuditEvent::new(AuditEventKind::SwarmCompleted, "Swarm completed")
            .with_data(serde_json::json!({"swarm_id":"support-escalation"})),
    ];

    let result = SwarmResult {
        config_id: "cfg-1".into(),
        status: SwarmStatus::Completed,
        dag,
        output: Some("customer notified".into()),
        metrics,
        audit_events,
        started_at: Utc::now(),
        completed_at: Some(Utc::now()),
    };

    let artifact =
        SwarmRunArtifact::from_swarm_result(&result, CoordinationPattern::Parallel, None);

    artifact.assert_metrics(1, 1).unwrap();
    artifact
        .assert_audit_event_kind(AuditEventKind::AgentReassigned)
        .unwrap();
    assert_eq!(artifact.swarm_status, Some(SwarmStatus::Completed));
    assert_eq!(artifact.output.as_deref(), Some("customer notified"));
    assert_eq!(artifact.metrics.as_ref().unwrap().agent_tokens["reviewer"], 120);
    assert!(artifact.to_markdown().contains("## Metrics"));
    assert!(artifact.to_markdown().contains("## Audit Trail"));
    assert!(artifact.to_markdown().contains("agent_reassigned"));
    assert!(artifact.to_json().contains("\"hitl_interventions\": 1"));
}
