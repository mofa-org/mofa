//! Debate: Hiring Decision
//!
//! Scenario: Two recruiters present opposing cases for a senior engineer
//! candidate — one argues hire, one argues pass. The hiring manager
//! (judge) reads both arguments and issues the final decision.
//!
//! Run: RUST_LOG=info cargo run -p swarm_coordination_patterns --bin debate_hiring_decision

use std::sync::Arc;

use anyhow::Result;
use futures::future::BoxFuture;
use mofa_foundation::swarm::{
    DebateScheduler, SubtaskDAG, SubtaskExecutorFn, SwarmScheduler, SwarmSubtask,
};
use mofa_kernel::agent::types::error::GlobalResult;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    println!("=== Debate: Hiring Decision ===");
    println!("DAG: pro_recruiter, con_recruiter (parallel) -> hiring_manager\n");

    let mut dag = SubtaskDAG::new("hiring-debate");

    let pro     = dag.add_task(SwarmSubtask::new("pro_recruiter", "Argue why the candidate should be hired based on their profile"));
    let con     = dag.add_task(SwarmSubtask::new("con_recruiter", "Argue why the candidate should not be hired based on their profile"));
    let manager = dag.add_task(SwarmSubtask::new("hiring_manager", "Read both arguments and issue the final hire/no-hire decision with rationale"));

    dag.add_dependency(pro, manager)?;
    dag.add_dependency(con, manager)?;

    let executor: SubtaskExecutorFn =
        Arc::new(move |_idx, task: SwarmSubtask| -> BoxFuture<'static, GlobalResult<String>> {
            let id = task.id.clone();
            let desc = task.description.clone();
            Box::pin(async move {
                let out = match id.as_str() {
                    "pro_recruiter" => "HIRE: 8 yrs distributed systems, led migration of monolith to microservices at scale (3M RPS), open-source contributor to tokio, strong system design, culture-fit score 9/10".to_string(),
                    "con_recruiter" => "PASS: no Rust production experience (our primary stack), asked for 30% above band, 3 job changes in 4 years, take-home assignment incomplete — missing error handling section".to_string(),
                    "hiring_manager" => {
                        let has_context = desc.contains("Debate Arguments");
                        format!(
                            "decision: CONDITIONAL_OFFER — {} Strong distributed systems background outweighs Rust gap (trainable in 3 months). Offer at band top. Require completed take-home before signing.",
                            if has_context { "reviewed both arguments." } else { "no arguments received." }
                        )
                    }
                    _ => "done".to_string(),
                };
                Ok(out.to_string())
            })
        });

    let summary = DebateScheduler::new().execute(&mut dag, executor).await?;

    println!("Recruiters (ran in parallel):");
    for r in &summary.results {
        if r.task_id == "hiring_manager" { continue; }
        let status = if r.outcome.is_success() { "OK" } else { "FAIL" };
        println!("  [{}] {} -> {}", status, r.task_id, r.outcome.output().unwrap_or("(no output)"));
    }

    if let Some(r) = summary.results.iter().find(|r| r.task_id == "hiring_manager") {
        let status = if r.outcome.is_success() { "OK" } else { "FAIL" };
        println!("\nHiring manager decision (receives both arguments injected):\n  [{}] {} -> {}",
            status, r.task_id, r.outcome.output().unwrap_or("(no output)"));
    }

    println!("\nsucceeded={} failed={} total_time={:?}",
        summary.succeeded, summary.failed, summary.total_wall_time);

    Ok(())
}
