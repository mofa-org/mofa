//! Routing: Medical Triage
//!
//! Scenario: A triage agent reads patient symptoms and identifies the
//! required specialty. The correct department agent (cardiology, neurology,
//! or general) handles the case; the other two are skipped.
//!
//! Run: RUST_LOG=info cargo run -p swarm_coordination_patterns --bin routing_medical_triage

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

    println!("=== Routing: Medical Triage ===");
    println!("Patient: chest pain, shortness of breath, left arm numbness");
    println!("Specialists: cardiology [cardiology], neurology [neurology], general [general]\n");

    let mut dag = SubtaskDAG::new("medical-triage");

    let triage = dag.add_task(SwarmSubtask::new("triage_agent", "Assess patient symptoms and identify the required medical specialty"));

    let mut cardio = SwarmSubtask::new("cardiology_dept", "Conduct cardiac assessment and order ECG, troponin panel");
    cardio.required_capabilities = vec!["cardiology".into()];
    let idx_cardio = dag.add_task(cardio);

    let mut neuro = SwarmSubtask::new("neurology_dept", "Conduct neurological assessment and order CT head");
    neuro.required_capabilities = vec!["neurology".into()];
    let idx_neuro = dag.add_task(neuro);

    let mut general = SwarmSubtask::new("general_dept", "Conduct general assessment and order basic labs");
    general.required_capabilities = vec!["general".into()];
    let idx_general = dag.add_task(general);

    dag.add_dependency(triage, idx_cardio)?;
    dag.add_dependency(triage, idx_neuro)?;
    dag.add_dependency(triage, idx_general)?;

    let executor: SubtaskExecutorFn =
        Arc::new(move |_idx, task: SwarmSubtask| -> BoxFuture<'static, GlobalResult<String>> {
            let id = task.id.clone();
            Box::pin(async move {
                let out = match id.as_str() {
                    "triage_agent"    => "symptoms indicate cardiology referral: chest pain + left arm radiation + dyspnoea — STEMI presentation, escalate immediately",
                    "cardiology_dept" => "cardiology_assessment: 12-lead ECG ordered, troponin T drawn, patient moved to resus bay, cath lab on standby — suspected STEMI",
                    "neurology_dept"  => "neurology_assessment: no focal deficits, no indication for CT head at this time",
                    "general_dept"    => "general_assessment: vital signs stable, no additional general workup required pending cardiac results",
                    _                 => "done",
                };
                Ok(out.to_string())
            })
        });

    let summary = RoutingScheduler::new().execute(&mut dag, executor).await?;

    println!("Triage output:");
    if let Some(r) = summary.results.iter().find(|r| r.task_id == "triage_agent") {
        println!("  [OK] triage_agent -> {}", r.outcome.output().unwrap_or(""));
    }

    println!("\nDepartment routing:");
    for r in &summary.results {
        if r.task_id == "triage_agent" { continue; }
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
