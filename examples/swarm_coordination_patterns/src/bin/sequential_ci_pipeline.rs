//! Sequential: CI/CD Pipeline
//!
//! Scenario: Four-stage pipeline — build → test → lint → deploy.
//! Each stage only starts after the previous one passes.
//!
//! Run: RUST_LOG=info cargo run -p swarm_coordination_patterns --bin sequential_ci_pipeline

use std::sync::Arc;

use anyhow::Result;
use futures::future::BoxFuture;
use mofa_foundation::swarm::{
    SequentialScheduler, SubtaskDAG, SubtaskExecutorFn, SwarmScheduler, SwarmSubtask,
};
use mofa_kernel::agent::types::error::GlobalResult;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    println!("=== Sequential: CI/CD Pipeline ===");
    println!("DAG: build -> test -> lint -> deploy\n");

    let mut dag = SubtaskDAG::new("ci-pipeline");

    let build  = dag.add_task(SwarmSubtask::new("build",  "Compile the project and produce artefacts"));
    let test   = dag.add_task(SwarmSubtask::new("test",   "Run the full test suite against build artefacts"));
    let lint   = dag.add_task(SwarmSubtask::new("lint",   "Run clippy and rustfmt checks on source"));
    let deploy = dag.add_task(SwarmSubtask::new("deploy", "Push artefacts to production environment"));

    dag.add_dependency(build, test)?;
    dag.add_dependency(test, lint)?;
    dag.add_dependency(lint, deploy)?;

    let executor: SubtaskExecutorFn =
        Arc::new(move |_idx, task: SwarmSubtask| -> BoxFuture<'static, GlobalResult<String>> {
            let id = task.id.clone();
            Box::pin(async move {
                let out = match id.as_str() {
                    "build"  => "build_ok: 142 files compiled, 0 errors, artefact: mofa-foundation.rlib (2.1 MB)",
                    "test"   => "tests_ok: 98 passed, 0 failed, 1 ignored — 0.26s",
                    "lint"   => "lint_ok: 0 clippy warnings, rustfmt diff empty",
                    "deploy" => "deploy_ok: pushed to prod-us-east-1, health check green, rollout 100%",
                    _        => "done",
                };
                Ok(out.to_string())
            })
        });

    let summary = SequentialScheduler::new().execute(&mut dag, executor).await?;

    println!("Pipeline stages (sequential guarantee):");
    for r in &summary.results {
        let status = if r.outcome.is_success() { "OK" } else { "FAIL" };
        println!("  [{}] {} -> {}", status, r.task_id, r.outcome.output().unwrap_or("(no output)"));
    }

    println!("\nsucceeded={} failed={} total_time={:?}",
        summary.succeeded, summary.failed, summary.total_wall_time);

    Ok(())
}
