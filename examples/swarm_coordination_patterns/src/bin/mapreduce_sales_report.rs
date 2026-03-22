//! MapReduce: Regional Sales Aggregation
//!
//! Scenario: Four regional sales agents process their territory data in
//! parallel (map phase). The reducer merges all regional figures into a
//! global quarterly revenue summary.
//!
//! Run: RUST_LOG=info cargo run -p swarm_coordination_patterns --bin mapreduce_sales_report

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

    println!("=== MapReduce: Regional Sales Aggregation ===");
    println!("DAG: region_na, region_emea, region_apac, region_latam (mappers) -> global_summary (reducer)\n");

    let mut dag = SubtaskDAG::new("sales-report");

    let na    = dag.add_task(SwarmSubtask::new("region_na",      "Compute Q4 revenue, deals closed, and churn for North America"));
    let emea  = dag.add_task(SwarmSubtask::new("region_emea",    "Compute Q4 revenue, deals closed, and churn for EMEA"));
    let apac  = dag.add_task(SwarmSubtask::new("region_apac",    "Compute Q4 revenue, deals closed, and churn for APAC"));
    let latam = dag.add_task(SwarmSubtask::new("region_latam",   "Compute Q4 revenue, deals closed, and churn for LATAM"));
    let r     = dag.add_task(SwarmSubtask::new("global_summary", "Aggregate all regional figures into a global Q4 revenue summary"));

    dag.add_dependency(na, r)?;
    dag.add_dependency(emea, r)?;
    dag.add_dependency(apac, r)?;
    dag.add_dependency(latam, r)?;

    let executor: SubtaskExecutorFn =
        Arc::new(move |_idx, task: SwarmSubtask| -> BoxFuture<'static, GlobalResult<String>> {
            let id = task.id.clone();
            let desc = task.description.clone();
            Box::pin(async move {
                let out = match id.as_str() {
                    "region_na"      => "na_q4: revenue=$4.2M, deals=318, churn=2.1%, top_product=mofa-enterprise".to_string(),
                    "region_emea"    => "emea_q4: revenue=$2.8M, deals=201, churn=3.4%, top_product=mofa-cloud".to_string(),
                    "region_apac"    => "apac_q4: revenue=$1.9M, deals=154, churn=1.8%, top_product=mofa-edge".to_string(),
                    "region_latam"   => "latam_q4: revenue=$0.7M, deals=63, churn=4.2%, top_product=mofa-cloud".to_string(),
                    "global_summary" => {
                        let has_context = desc.contains("Map Phase Outputs");
                        format!(
                            "global_q4_summary: {} — total_revenue=$9.6M, total_deals=736, blended_churn=2.9%. Best region: NA. Fastest growth: APAC (+38% QoQ).",
                            if has_context { "all 4 regions consolidated" } else { "no regional data" }
                        )
                    }
                    _ => "done".to_string(),
                };
                Ok(out.to_string())
            })
        });

    let summary = MapReduceScheduler::new().execute(&mut dag, executor).await?;

    println!("Map phase (parallel regional agents):");
    for r in &summary.results {
        if r.task_id == "global_summary" { continue; }
        let status = if r.outcome.is_success() { "OK" } else { "FAIL" };
        println!("  [{}] {} -> {}", status, r.task_id, r.outcome.output().unwrap_or("(no output)"));
    }

    if let Some(r) = summary.results.iter().find(|r| r.task_id == "global_summary") {
        let status = if r.outcome.is_success() { "OK" } else { "FAIL" };
        println!("\nReduce phase (global aggregator):\n  [{}] {} -> {}",
            status, r.task_id, r.outcome.output().unwrap_or("(no output)"));
    }

    println!("\nsucceeded={} failed={} total_time={:?}",
        summary.succeeded, summary.failed, summary.total_wall_time);

    Ok(())
}
