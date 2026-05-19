//! Supervision: Multi-Region Deployment with Failure Recovery
//!
//! Scenario: Deploy a new release to three regions simultaneously.
//! The us-west-2 region fails (health check timeout). The ops supervisor
//! always runs after all workers and issues a recovery plan.
//!
//! Run: RUST_LOG=info cargo run -p swarm_coordination_patterns --bin supervision_deployment_pipeline

use std::sync::Arc;

use anyhow::Result;
use futures::future::BoxFuture;
use mofa_foundation::swarm::{
    SubtaskDAG, SubtaskExecutorFn, SupervisionScheduler, SwarmScheduler, SwarmSubtask,
};
use mofa_kernel::agent::types::error::GlobalResult;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    println!("=== Supervision: Multi-Region Deployment ===");
    println!("Workers: deploy_us_east (OK), deploy_us_west (FAILS), deploy_eu_west (OK)");
    println!("Supervisor always runs and issues recovery plan\n");

    let mut dag = SubtaskDAG::new("deployment");

    let w1 = dag.add_task(SwarmSubtask::new("deploy_us_east", "Deploy v2.1 to us-east-1 and validate health checks"));
    let w2 = dag.add_task(SwarmSubtask::new("deploy_us_west", "Deploy v2.1 to us-west-2 and validate health checks"));
    let w3 = dag.add_task(SwarmSubtask::new("deploy_eu_west", "Deploy v2.1 to eu-west-1 and validate health checks"));
    let sv = dag.add_task(SwarmSubtask::new("ops_supervisor", "Review all region outcomes and issue rollback or recovery plan"));

    dag.add_dependency(w1, sv)?;
    dag.add_dependency(w2, sv)?;
    dag.add_dependency(w3, sv)?;

    let executor: SubtaskExecutorFn =
        Arc::new(move |_idx, task: SwarmSubtask| -> BoxFuture<'static, GlobalResult<String>> {
            let id = task.id.clone();
            let desc = task.description.clone();
            Box::pin(async move {
                match id.as_str() {
                    "deploy_us_east" => Ok("deploy_ok: us-east-1 v2.1 healthy, 3/3 instances passing, latency p99=42ms".into()),
                    "deploy_us_west" => Err(mofa_kernel::agent::types::error::GlobalError::runtime(
                        "deploy_failed: us-west-2 health check timeout after 30s — instances stuck in ELB drain state",
                    )),
                    "deploy_eu_west" => Ok("deploy_ok: eu-west-1 v2.1 healthy, 3/3 instances passing, latency p99=38ms".into()),
                    "ops_supervisor" => {
                        let has_context = desc.contains("Worker Results");
                        Ok(format!(
                            "recovery_plan: {} — us-east-1 and eu-west-1 stable (2/3 regions). Action: rollback us-west-2 to v2.0 (in progress), trigger PagerDuty P2, schedule re-deploy post-drain investigation. Global traffic shifted to east+eu.",
                            if has_context { "all region outcomes reviewed." } else { "no outcome data." }
                        ))
                    }
                    _ => Ok("done".into()),
                }
            })
        });

    let summary = SupervisionScheduler::new().execute(&mut dag, executor).await?;

    println!("Region deployments:");
    for r in &summary.results {
        if r.task_id == "ops_supervisor" { continue; }
        let label = if r.outcome.is_success() { "SUCCESS" } else { "FAILED " };
        let detail = r.outcome.output().unwrap_or_else(|| match &r.outcome {
            mofa_foundation::swarm::TaskOutcome::Failure(e) => e.as_str(),
            _ => "",
        });
        println!("  [{}] {} -> {}", label, r.task_id, detail);
    }

    if let Some(r) = summary.results.iter().find(|r| r.task_id == "ops_supervisor") {
        println!("\nOps supervisor (always runs):\n  [OK] {} -> {}",
            r.task_id, r.outcome.output().unwrap_or("(no output)"));
    }

    println!("\nsucceeded={} failed={} total_time={:?}",
        summary.succeeded, summary.failed, summary.total_wall_time);

    Ok(())
}
