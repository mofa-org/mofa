//! Consensus: Pull Request Approval
//!
//! Scenario: Three code reviewers independently vote approve or request-changes
//! on a pull request. The merge bot reads all votes and applies majority
//! rule to decide whether to merge or block.
//!
//! Run: RUST_LOG=info cargo run -p swarm_coordination_patterns --bin consensus_code_approval

use std::sync::Arc;

use anyhow::Result;
use futures::future::BoxFuture;
use mofa_foundation::swarm::{
    ConsensusScheduler, SubtaskDAG, SubtaskExecutorFn, SwarmScheduler, SwarmSubtask,
};
use mofa_kernel::agent::types::error::GlobalResult;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    println!("=== Consensus: Pull Request Approval ===");
    println!("DAG: reviewer_a, reviewer_b, reviewer_c (voters) -> merge_bot (aggregator)\n");

    let mut dag = SubtaskDAG::new("pr-approval");

    let ra = dag.add_task(SwarmSubtask::new("reviewer_a", "Review PR #1398 for correctness and safety"));
    let rb = dag.add_task(SwarmSubtask::new("reviewer_b", "Review PR #1398 for style and test coverage"));
    let rc = dag.add_task(SwarmSubtask::new("reviewer_c", "Review PR #1398 for performance implications"));
    let mb = dag.add_task(SwarmSubtask::new("merge_bot",  "Apply majority vote to decide merge or block for PR #1398"));

    dag.add_dependency(ra, mb)?;
    dag.add_dependency(rb, mb)?;
    dag.add_dependency(rc, mb)?;

    let executor: SubtaskExecutorFn =
        Arc::new(move |_idx, task: SwarmSubtask| -> BoxFuture<'static, GlobalResult<String>> {
            let id = task.id.clone();
            let desc = task.description.clone();
            Box::pin(async move {
                let out = match id.as_str() {
                    "reviewer_a" => "approve".to_string(),
                    "reviewer_b" => "approve".to_string(),
                    "reviewer_c" => "request_changes: missing benchmark for MapReduceScheduler parallel wave".to_string(),
                    "merge_bot"  => {
                        let has_context = desc.contains("Voter Outputs");
                        format!(
                            "merge_decision: MERGE — {} 2/3 reviewers approved (majority). reviewer_c's benchmark concern logged as follow-up issue #1401.",
                            if has_context { "all votes received." } else { "no votes." }
                        )
                    }
                    _ => "done".to_string(),
                };
                Ok(out)
            })
        });

    let summary = ConsensusScheduler::new().execute(&mut dag, executor).await?;

    println!("Reviewers (ran in parallel):");
    for r in &summary.results {
        if r.task_id == "merge_bot" { continue; }
        let status = if r.outcome.is_success() { "OK" } else { "FAIL" };
        println!("  [{}] {} -> {}", status, r.task_id, r.outcome.output().unwrap_or("(no output)"));
    }

    if let Some(r) = summary.results.iter().find(|r| r.task_id == "merge_bot") {
        let status = if r.outcome.is_success() { "OK" } else { "FAIL" };
        println!("\nMerge bot decision (receives all votes injected):\n  [{}] {} -> {}",
            status, r.task_id, r.outcome.output().unwrap_or("(no output)"));
    }

    println!("\nsucceeded={} failed={} total_time={:?}",
        summary.succeeded, summary.failed, summary.total_wall_time);

    Ok(())
}
