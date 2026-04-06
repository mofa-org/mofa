//! MapReduce: Distributed Log Analysis
//!
//! Scenario: Four server log shards are parsed in parallel (map phase).
//! Each shard extracts its error summary. The reducer aggregates all
//! shard summaries into a single incident report.
//!
//! Run: RUST_LOG=info cargo run -p swarm_coordination_patterns --bin mapreduce_log_analysis

use std::sync::Arc;

use anyhow::Result;
use futures::future::BoxFuture;
use mofa_foundation::swarm::{
    MapReduceScheduler, SubtaskDAG, SubtaskExecutorFn, SwarmScheduler, SwarmSubtask,
};
use mofa_kernel::agent::types::error::GlobalResult;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    println!("=== MapReduce: Distributed Log Analysis ===");
    println!("DAG: shard_1, shard_2, shard_3, shard_4 (mappers) -> incident_report (reducer)\n");

    let mut dag = SubtaskDAG::new("log-analysis");

    let s1 = dag.add_task(SwarmSubtask::new("shard_1", "Parse /var/log/app/app.log.1 for ERROR and WARN entries"));
    let s2 = dag.add_task(SwarmSubtask::new("shard_2", "Parse /var/log/app/app.log.2 for ERROR and WARN entries"));
    let s3 = dag.add_task(SwarmSubtask::new("shard_3", "Parse /var/log/app/app.log.3 for ERROR and WARN entries"));
    let s4 = dag.add_task(SwarmSubtask::new("shard_4", "Parse /var/log/app/app.log.4 for ERROR and WARN entries"));
    let r  = dag.add_task(SwarmSubtask::new("incident_report", "Aggregate all shard summaries into a prioritised incident report"));

    dag.add_dependency(s1, r)?;
    dag.add_dependency(s2, r)?;
    dag.add_dependency(s3, r)?;
    dag.add_dependency(s4, r)?;

    let executor: SubtaskExecutorFn =
        Arc::new(move |_idx, task: SwarmSubtask| -> BoxFuture<'static, GlobalResult<String>> {
            let id = task.id.clone();
            let desc = task.description.clone();
            Box::pin(async move {
                let out = match id.as_str() {
                    "shard_1" => "shard_1_summary: 12 ERRORs (db_connection_timeout x8, auth_failure x4), 31 WARNs".to_string(),
                    "shard_2" => "shard_2_summary: 3 ERRORs (disk_full x3), 5 WARNs — spike at 02:14 UTC".to_string(),
                    "shard_3" => "shard_3_summary: 0 ERRORs, 2 WARNs — clean period".to_string(),
                    "shard_4" => "shard_4_summary: 7 ERRORs (oom_killed x7), 18 WARNs — memory pressure detected".to_string(),
                    "incident_report" => {
                        let has_context = desc.contains("Map Phase Outputs");
                        format!(
                            "incident_report_ok: {} — top issues: db_connection_timeout (8), oom_killed (7), disk_full (3). Recommend: scale db pool, add swap, expand disk.",
                            if has_context { "all 4 shards aggregated" } else { "no shard context" }
                        )
                    }
                    _ => "done".to_string(),
                };
                Ok(out.to_string())
            })
        });

    let summary = MapReduceScheduler::new().execute(&mut dag, executor).await?;

    println!("Map phase (parallel shard parsers):");
    for r in &summary.results {
        if r.task_id == "incident_report" { continue; }
        let status = if r.outcome.is_success() { "OK" } else { "FAIL" };
        println!("  [{}] {} -> {}", status, r.task_id, r.outcome.output().unwrap_or("(no output)"));
    }

    if let Some(r) = summary.results.iter().find(|r| r.task_id == "incident_report") {
        let status = if r.outcome.is_success() { "OK" } else { "FAIL" };
        println!("\nReduce phase (incident aggregator):\n  [{}] {} -> {}",
            status, r.task_id, r.outcome.output().unwrap_or("(no output)"));
    }

    println!("\nsucceeded={} failed={} total_time={:?}",
        summary.succeeded, summary.failed, summary.total_wall_time);

    Ok(())
}
