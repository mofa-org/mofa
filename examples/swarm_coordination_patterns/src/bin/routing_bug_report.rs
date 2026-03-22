//! Routing: Bug Report Dispatch
//!
//! Scenario: A classifier reads an incoming bug report and identifies
//! which engineering team owns it (backend, frontend, or infra).
//! The correct team handles the report; the others are skipped.
//!
//! Run: RUST_LOG=info cargo run -p swarm_coordination_patterns --bin routing_bug_report

use std::sync::Arc;

use anyhow::Result;
use futures::future::BoxFuture;
use mofa_foundation::swarm::{
    RoutingScheduler, SubtaskDAG, SubtaskExecutorFn, SwarmScheduler, SwarmSubtask,
};
use mofa_kernel::agent::types::error::GlobalResult;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    println!("=== Routing: Bug Report Dispatch ===");
    println!("Bug: 'API returns 500 on POST /swarm/execute when DAG has a cycle'");
    println!("Teams: backend_team [backend], frontend_team [frontend], infra_team [infra]\n");

    let mut dag = SubtaskDAG::new("bug-routing");

    let classifier = dag.add_task(SwarmSubtask::new("bug_classifier", "Analyse the bug report and identify the owning engineering team"));

    let mut backend = SwarmSubtask::new("backend_team", "Triage and assign the bug to a backend engineer");
    backend.required_capabilities = vec!["backend".into()];
    let idx_backend = dag.add_task(backend);

    let mut frontend = SwarmSubtask::new("frontend_team", "Triage and assign the bug to a frontend engineer");
    frontend.required_capabilities = vec!["frontend".into()];
    let idx_frontend = dag.add_task(frontend);

    let mut infra = SwarmSubtask::new("infra_team", "Triage and assign the bug to an infra engineer");
    infra.required_capabilities = vec!["infra".into()];
    let idx_infra = dag.add_task(infra);

    dag.add_dependency(classifier, idx_backend)?;
    dag.add_dependency(classifier, idx_frontend)?;
    dag.add_dependency(classifier, idx_infra)?;

    let executor: SubtaskExecutorFn =
        Arc::new(move |_idx, task: SwarmSubtask| -> BoxFuture<'static, GlobalResult<String>> {
            let id = task.id.clone();
            Box::pin(async move {
                let out = match id.as_str() {
                    "bug_classifier" => "team: backend — cycle detection in DAG validation layer, 500 response from /swarm/execute endpoint, stack trace points to SubtaskDAG::add_dependency",
                    "backend_team"   => "bug_triaged: assigned to @nityam (backend-platform), severity=HIGH, milestone=v2.1.1 hotfix, root cause: cycle check only runs at task execution time, not at add_dependency — fix: add petgraph cycle detection on edge insertion",
                    "frontend_team"  => "not applicable",
                    "infra_team"     => "not applicable",
                    _                => "done",
                };
                Ok(out.to_string())
            })
        });

    let summary = RoutingScheduler::new().execute(&mut dag, executor).await?;

    println!("Classifier output:");
    if let Some(r) = summary.results.iter().find(|r| r.task_id == "bug_classifier") {
        println!("  [OK] bug_classifier -> {}", r.outcome.output().unwrap_or(""));
    }

    println!("\nTeam routing:");
    for r in &summary.results {
        if r.task_id == "bug_classifier" { continue; }
        let label = match &r.outcome {
            mofa_foundation::swarm::TaskOutcome::Success(s) => format!("[SELECTED] {}", s),
            mofa_foundation::swarm::TaskOutcome::Skipped(_) => "[SKIPPED]".into(),
            mofa_foundation::swarm::TaskOutcome::Failure(e) => format!("[FAIL] {}", e),
        };
        println!("  {} -> {}", r.task_id, label);
    }

    println!("\nsucceeded={} skipped={} total_time={:?}",
        summary.succeeded, summary.skipped, summary.total_wall_time);

    Ok(())
}
