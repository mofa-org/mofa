use std::sync::Arc;

use anyhow::Result;
use futures::future::BoxFuture;
use mofa_foundation::swarm::{
    ParallelScheduler, SubtaskDAG, SubtaskExecutorFn, SwarmScheduler, SwarmSubtask,
};
use mofa_kernel::agent::types::error::GlobalResult;

fn reviewer_executor() -> SubtaskExecutorFn {
    Arc::new(move |_idx, task: SwarmSubtask| -> BoxFuture<'static, GlobalResult<String>> {
        let id = task.id.clone();
        Box::pin(async move {
            let output = match id.as_str() {
                "lint_check" => "lint_passed: 0 errors, 3 style warnings (unused imports)".to_string(),
                "security_scan" => "security_passed: no CVEs found, 1 low-severity advisory (outdated dep)".to_string(),
                "perf_check" => "perf_passed: O(n^2) hotspot in sort loop flagged, memory allocation within bounds".to_string(),
                "review_summary" => {
                    let desc = task.description.clone();
                    format!("review_complete: aggregated {} chars of reviewer feedback", desc.len())
                }
                _ => format!("{}: processed", id),
            };
            Ok(output)
        })
    })
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let mut dag = SubtaskDAG::new("parallel-code-review");

    let lint = dag.add_task(SwarmSubtask::new("lint_check", "Run linter across all modified files"));
    let security = dag.add_task(SwarmSubtask::new("security_scan", "Scan dependencies and code for vulnerabilities"));
    let perf = dag.add_task(SwarmSubtask::new("perf_check", "Profile hot paths and flag regressions"));
    let summary = dag.add_task(SwarmSubtask::new("review_summary", "Aggregate all reviewer outputs into final verdict"));

    dag.add_dependency(lint, summary)?;
    dag.add_dependency(security, summary)?;
    dag.add_dependency(perf, summary)?;

    println!("=== Parallel: Code Review Pipeline ===");
    println!("DAG: lint_check, security_scan, perf_check (parallel) -> review_summary\n");

    let exec_summary = ParallelScheduler::new()
        .execute(&mut dag, reviewer_executor())
        .await?;

    let parallel_tasks: Vec<_> = exec_summary.results.iter()
        .filter(|r| r.task_id != "review_summary")
        .collect();
    let sink = exec_summary.results.iter().find(|r| r.task_id == "review_summary");

    println!("Parallel reviewers (ran concurrently):");
    for result in &parallel_tasks {
        let status = if result.outcome.is_success() { "OK" } else { "FAIL" };
        println!("  [{}] {} -> {}", status, result.task_id, result.outcome.output().unwrap_or("(no output)"));
    }

    if let Some(s) = sink {
        let status = if s.outcome.is_success() { "OK" } else { "FAIL" };
        println!("\nAggregation sink:");
        println!("  [{}] {} -> {}", status, s.task_id, s.outcome.output().unwrap_or("(no output)"));
    }

    println!("\nSummary: succeeded={} failed={} total_wall_time={:?}",
        exec_summary.succeeded, exec_summary.failed, exec_summary.total_wall_time);

    Ok(())
}
