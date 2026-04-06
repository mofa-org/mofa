//! Parallel: Multi-Source Data Ingestion
//!
//! Scenario: Pull data from four external APIs simultaneously (Stripe,
//! Salesforce, Jira, GitHub). All four ingestion agents run in parallel
//! then the aggregated payload is ready for the ETL layer.
//!
//! Run: RUST_LOG=info cargo run -p swarm_coordination_patterns --bin parallel_data_ingestion

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

    println!("=== Parallel: Multi-Source Data Ingestion ===");
    println!("DAG: ingest_stripe, ingest_salesforce, ingest_jira, ingest_github (all parallel)\n");

    let mut dag = SubtaskDAG::new("data-ingestion");

    dag.add_task(SwarmSubtask::new("ingest_stripe",     "Fetch last 24h of payment events from Stripe API"));
    dag.add_task(SwarmSubtask::new("ingest_salesforce", "Pull updated opportunity records from Salesforce CRM"));
    dag.add_task(SwarmSubtask::new("ingest_jira",       "Retrieve all tickets transitioned in the last 24h from Jira"));
    dag.add_task(SwarmSubtask::new("ingest_github",     "Collect merged PRs and closed issues from the mofa-org GitHub org"));

    let executor: SubtaskExecutorFn =
        Arc::new(move |_idx, task: SwarmSubtask| -> BoxFuture<'static, GlobalResult<String>> {
            let id = task.id.clone();
            Box::pin(async move {
                let out = match id.as_str() {
                    "ingest_stripe"     => "stripe_ok: 1,240 events ingested, $48,320 total volume, 3 disputes flagged",
                    "ingest_salesforce" => "salesforce_ok: 87 opportunities updated, 12 new leads, pipeline delta +$210k",
                    "ingest_jira"       => "jira_ok: 34 tickets closed, 18 moved to IN_REVIEW, 5 reopened",
                    "ingest_github"     => "github_ok: 9 PRs merged, 22 issues closed, 4 new contributors",
                    _                   => "done",
                };
                Ok(out.to_string())
            })
        });

    let summary = ParallelScheduler::new().execute(&mut dag, executor).await?;

    println!("Ingestion agents (ran concurrently):");
    for r in &summary.results {
        let status = if r.outcome.is_success() { "OK" } else { "FAIL" };
        println!("  [{}] {} -> {}", status, r.task_id, r.outcome.output().unwrap_or("(no output)"));
    }

    println!("\nsucceeded={} failed={} total_time={:?}",
        summary.succeeded, summary.failed, summary.total_wall_time);

    Ok(())
}
