use std::sync::Arc;

use anyhow::Result;
use futures::future::BoxFuture;
use mofa_foundation::swarm::{
    SubtaskDAG, SubtaskExecutorFn, SupervisionScheduler, SwarmScheduler, SwarmSubtask,
};
use mofa_kernel::agent::types::error::{GlobalError, GlobalResult};

fn shard_executor() -> SubtaskExecutorFn {
    Arc::new(move |_idx, task: SwarmSubtask| -> BoxFuture<'static, GlobalResult<String>> {
        let id = task.id.clone();
        let desc = task.description.clone();
        Box::pin(async move {
            match id.as_str() {
                "shard_a_processor" => Ok("shard_a_complete: 1M records indexed, checksum OK".to_string()),
                "shard_b_processor" => Err(GlobalError::runtime("shard_b_unavailable: disk I/O timeout after 30s")),
                "shard_c_processor" => Ok("shard_c_complete: 980K records indexed, 20K skipped (invalid schema)".to_string()),
                "recovery_coordinator" => {
                    let has_context = desc.contains("Worker Results");
                    let failed_count = desc.matches("FAILED").count();
                    let success_count = desc.matches("SUCCESS").count();
                    Ok(format!(
                        "recovery_plan: detected {} failed shards, {} successful. \
                         Action: re-queue shard_b with exponential backoff, alert on-call. \
                         Context injected: {}",
                        failed_count,
                        success_count,
                        has_context
                    ))
                }
                _ => Ok(format!("{}: processed", id)),
            }
        })
    })
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let mut dag = SubtaskDAG::new("resilient-data-pipeline");

    let shard_a = dag.add_task(SwarmSubtask::new(
        "shard_a_processor",
        "Process partition A of the distributed dataset",
    ));
    let shard_b = dag.add_task(SwarmSubtask::new(
        "shard_b_processor",
        "Process partition B of the distributed dataset",
    ));
    let shard_c = dag.add_task(SwarmSubtask::new(
        "shard_c_processor",
        "Process partition C of the distributed dataset",
    ));
    let supervisor = dag.add_task(SwarmSubtask::new(
        "recovery_coordinator",
        "Review worker outcomes and coordinate failure recovery",
    ));

    dag.add_dependency(shard_a, supervisor)?;
    dag.add_dependency(shard_b, supervisor)?;
    dag.add_dependency(shard_c, supervisor)?;

    println!("=== Supervision: Resilient Distributed Data Pipeline ===");
    println!("Workers: shard_a (OK), shard_b (FAILS), shard_c (OK)");
    println!("Supervisor always runs and receives worker outcomes in its context\n");

    let summary = SupervisionScheduler::new()
        .execute(&mut dag, shard_executor())
        .await?;

    use mofa_foundation::swarm::TaskOutcome;

    println!("Worker results:");
    for r in &summary.results {
        if r.task_id == "recovery_coordinator" {
            continue;
        }
        let status = match &r.outcome {
            TaskOutcome::Success(_) => "SUCCESS",
            TaskOutcome::Failure(_) => "FAILED ",
            TaskOutcome::Skipped(_) => "SKIPPED",
        };
        let detail = match &r.outcome {
            TaskOutcome::Success(s) => s.as_str(),
            TaskOutcome::Failure(s) => s.as_str(),
            TaskOutcome::Skipped(s) => s.as_str(),
        };
        println!("  [{}] {} -> {}", status, r.task_id, detail);
    }

    let supervisor_result = summary.results.iter().find(|r| r.task_id == "recovery_coordinator");
    if let Some(r) = supervisor_result {
        let status = if r.outcome.is_success() { "OK" } else { "FAIL" };
        println!("\nSupervisor (receives SUCCESS/FAILED context for each worker):");
        println!("  [{}] {} -> {}", status, r.task_id, r.outcome.output().unwrap_or("(no output)"));
    }

    println!("\nSummary: succeeded={} failed={} total_wall_time={:?}",
        summary.succeeded, summary.failed, summary.total_wall_time);

    Ok(())
}
