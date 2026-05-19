//! Parallel: Test Suite Runner
//!
//! Scenario: Run unit, integration, e2e, and performance test suites
//! simultaneously. All suites execute in parallel; results are merged
//! into a single quality gate report.
//!
//! Run: RUST_LOG=info cargo run -p swarm_coordination_patterns --bin parallel_test_runner

use std::sync::Arc;

use anyhow::Result;
use futures::future::BoxFuture;
use mofa_foundation::swarm::{
    ParallelScheduler, SubtaskDAG, SubtaskExecutorFn, SwarmScheduler, SwarmSubtask,
};
use mofa_kernel::agent::types::error::GlobalResult;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    println!("=== Parallel: Test Suite Runner ===");
    println!("DAG: unit_tests, integration_tests, e2e_tests, perf_tests (all parallel)\n");

    let mut dag = SubtaskDAG::new("test-runner");

    dag.add_task(SwarmSubtask::new("unit_tests",        "Run all unit tests in mofa-foundation and mofa-kernel"));
    dag.add_task(SwarmSubtask::new("integration_tests", "Run inter-crate integration tests with in-memory stores"));
    dag.add_task(SwarmSubtask::new("e2e_tests",         "Run end-to-end swarm orchestration tests against local runtime"));
    dag.add_task(SwarmSubtask::new("perf_tests",        "Run criterion benchmarks for scheduler hot paths"));

    let executor: SubtaskExecutorFn =
        Arc::new(move |_idx, task: SwarmSubtask| -> BoxFuture<'static, GlobalResult<String>> {
            let id = task.id.clone();
            Box::pin(async move {
                let out = match id.as_str() {
                    "unit_tests"        => "unit_ok: 98 passed, 0 failed — 0.26s",
                    "integration_tests" => "integration_ok: 14 passed, 0 failed — 1.83s",
                    "e2e_tests"         => "e2e_ok: 6 scenarios passed (sequential, parallel, mapreduce, debate, consensus, routing) — 4.12s",
                    "perf_tests"        => "perf_ok: parallel_scheduler p99=1.2ms, sequential_scheduler p99=0.4ms — within SLA",
                    _                   => "done",
                };
                Ok(out.to_string())
            })
        });

    let summary = ParallelScheduler::new().execute(&mut dag, executor).await?;

    println!("Test suites (ran concurrently):");
    for r in &summary.results {
        let status = if r.outcome.is_success() { "OK" } else { "FAIL" };
        println!("  [{}] {} -> {}", status, r.task_id, r.outcome.output().unwrap_or("(no output)"));
    }

    println!("\nsucceeded={} failed={} total_time={:?}",
        summary.succeeded, summary.failed, summary.total_wall_time);

    Ok(())
}
