use mofa_foundation::swarm::{
    AuditEntry, AuditEvent, AuditEventKind, RiskLevel, SubtaskDAG, SwarmAuditLog, SwarmAuditor,
    SwarmSubtask,
};

// prints every event to stdout as it arrives
struct ConsoleAuditor;

impl SwarmAuditor for ConsoleAuditor {
    fn on_entry(&self, entry: &AuditEntry) {
        println!(
            "[{}] {:?} — {}",
            entry.event.timestamp.format("%H:%M:%S%.3f"),
            entry.event.kind,
            entry.event.description,
        );
    }
}

fn build_dag() -> SubtaskDAG {
    let mut dag = SubtaskDAG::new("research-pipeline");
    let fetch = dag.add_task(
        SwarmSubtask::new("fetch", "fetch raw data from api")
            .with_capabilities(vec!["web_search".into()])
            .with_complexity(0.4),
    );
    let summarise = dag.add_task(
        SwarmSubtask::new("summarise", "summarise fetched content")
            .with_capabilities(vec!["summarization".into()])
            .with_complexity(0.6),
    );
    let review = dag.add_task(
        SwarmSubtask::new("review", "critical review of summary")
            .with_capabilities(vec!["analysis".into()])
            .with_risk_level(RiskLevel::High)
            .with_complexity(0.8),
    );
    dag.add_dependency(fetch, summarise).unwrap();
    dag.add_dependency(summarise, review).unwrap();
    dag
}

#[tokio::main]
async fn main() {
    let log = SwarmAuditLog::new().with_auditor(ConsoleAuditor);

    let dag = build_dag();

    println!("=== swarm audit log demo ===\n");

    // simulate admission gate check
    log.record(
        AuditEvent::new(AuditEventKind::AdmissionChecked, "dag admitted by admission gate")
            .with_data(serde_json::json!({
                "dag_id": dag.id,
                "task_count": dag.task_count(),
                "policies_checked": 4,
                "denied": 0,
                "warned": 0,
            })),
    );

    // simulate pattern selection
    log.record(
        AuditEvent::new(AuditEventKind::PatternSelected, "sequential selected for linear dag")
            .with_data(serde_json::json!({
                "dag_id": dag.id,
                "pattern": "sequential",
            })),
    );

    // simulate scheduler lifecycle + task events
    log.record(
        AuditEvent::new(AuditEventKind::SchedulerStarted, "sequential scheduler starting")
            .with_data(serde_json::json!({
                "dag_id": dag.id,
                "task_count": dag.task_count(),
            })),
    );

    for (_, task) in dag.all_tasks() {
        log.record(
            AuditEvent::new(AuditEventKind::SubtaskStarted, &task.description)
                .with_data(serde_json::json!({ "task_id": task.id })),
        );

        if task.risk_level.requires_hitl() {
            log.record(
                AuditEvent::new(AuditEventKind::HITLRequested, "high-risk task held for review")
                    .with_data(serde_json::json!({
                        "task_id": task.id,
                        "risk_level": format!("{:?}", task.risk_level),
                    })),
            );
            log.record(
                AuditEvent::new(AuditEventKind::HITLDecision, "reviewer approved")
                    .with_data(serde_json::json!({
                        "task_id": task.id,
                        "decision": "approved",
                    })),
            );
        }

        log.record(
            AuditEvent::new(AuditEventKind::SubtaskCompleted, &task.description)
                .with_data(serde_json::json!({
                    "task_id": task.id,
                    "wall_time_ms": 120,
                })),
        );
    }

    log.record(
        AuditEvent::new(AuditEventKind::SchedulerCompleted, "scheduler finished")
            .with_data(serde_json::json!({
                "dag_id": dag.id,
                "succeeded": dag.task_count(),
                "failed": 0,
            })),
    );

    println!("\n=== summary ===");
    println!("total entries : {}", log.len());
    println!(
        "subtasks started   : {}",
        log.entries_by_kind(&AuditEventKind::SubtaskStarted).len()
    );
    println!(
        "subtasks completed : {}",
        log.entries_by_kind(&AuditEventKind::SubtaskCompleted).len()
    );
    println!(
        "hitl interventions : {}",
        log.entries_by_kind(&AuditEventKind::HITLRequested).len()
    );

    println!("\n=== audit trail (json) ===");
    let events = log.to_audit_events();
    println!("{}", serde_json::to_string_pretty(&events).unwrap());
}
