//! SwarmTelemetry demo — enriched span fields and child task spans.
//!
//! Scenario: 4 independent tasks run in parallel. After execution the
//! scheduler span carries succeeded/failed/skipped/wall_time_ms/peak_concurrency
//! as structured fields, and each task has its own child `swarm.task` span.
//!
//! Run: RUST_LOG=info cargo run -p swarm_coordination_patterns --bin swarm_telemetry_demo

use std::sync::Arc;

use anyhow::Result;
use futures::future::BoxFuture;
use mofa_foundation::swarm::{ParallelScheduler, SubtaskDAG, SwarmScheduler, SwarmSubtask};
use mofa_kernel::agent::types::error::GlobalResult;
use petgraph::graph::NodeIndex;
use tracing::info;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter("info")
        .init();

    println!("=== SwarmTelemetry: Parallel Execution with Enriched Spans ===\n");

    let mut dag = SubtaskDAG::new("telemetry-demo");
    dag.add_task(SwarmSubtask::new("ingest-api-1", "Ingest records from API endpoint 1"));
    dag.add_task(SwarmSubtask::new("ingest-api-2", "Ingest records from API endpoint 2"));
    dag.add_task(SwarmSubtask::new("ingest-api-3", "Ingest records from API endpoint 3"));
    dag.add_task(SwarmSubtask::new("ingest-api-4", "Ingest records from API endpoint 4"));

    let executor: Arc<dyn Fn(NodeIndex, SwarmSubtask) -> BoxFuture<'static, GlobalResult<String>> + Send + Sync> =
        Arc::new(move |_idx, task: SwarmSubtask| {
            Box::pin(async move {
                // Simulate variable ingestion time
                tokio::time::sleep(std::time::Duration::from_millis(20)).await;
                Ok(format!("{}: ingested 1 200 records", task.id))
            })
        });

    let summary = ParallelScheduler::new()
        .execute(&mut dag, executor)
        .await?;

    println!("succeeded={} failed={} skipped={}", summary.succeeded, summary.failed, summary.skipped);
    println!("wall_time_ms={}", summary.total_wall_time.as_millis());
    println!("peak_concurrency={}", summary.peak_concurrency());
    println!();

    for r in summary.timeline() {
        info!(task_id = %r.task_id, outcome = ?r.outcome, wall_ms = r.wall_time.as_millis(), "task result");
        println!("  [{}] {}ms", r.task_id, r.wall_time.as_millis());
    }

    println!("\n[observe in logs] each task emits a child `swarm.task` span");
    println!("[observe in logs] parent span carries succeeded/failed/skipped/wall_time_ms/peak_concurrency");

    Ok(())
}
