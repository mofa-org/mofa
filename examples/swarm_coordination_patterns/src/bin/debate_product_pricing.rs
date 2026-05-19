//! Debate: Product Pricing Strategy
//!
//! Scenario: Growth team argues for a lower price to maximise adoption;
//! Revenue team argues for a higher price to maximise margin. The CEO
//! reads both cases and sets the final pricing strategy.
//!
//! Run: RUST_LOG=info cargo run -p swarm_coordination_patterns --bin debate_product_pricing

use std::sync::Arc;

use anyhow::Result;
use futures::future::BoxFuture;
use mofa_foundation::swarm::{
    DebateScheduler, SubtaskDAG, SubtaskExecutorFn, SwarmScheduler, SwarmSubtask,
};
use mofa_kernel::agent::types::error::GlobalResult;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    println!("=== Debate: Product Pricing Strategy ===");
    println!("DAG: growth_team, revenue_team (parallel) -> ceo\n");

    let mut dag = SubtaskDAG::new("pricing-debate");

    let growth  = dag.add_task(SwarmSubtask::new("growth_team",  "Argue for a lower price point to maximise user acquisition and market share"));
    let revenue = dag.add_task(SwarmSubtask::new("revenue_team", "Argue for a higher price point to maximise per-seat margin and signal premium quality"));
    let ceo     = dag.add_task(SwarmSubtask::new("ceo",          "Review both pricing arguments and set the final go-to-market price"));

    dag.add_dependency(growth, ceo)?;
    dag.add_dependency(revenue, ceo)?;

    let executor: SubtaskExecutorFn =
        Arc::new(move |_idx, task: SwarmSubtask| -> BoxFuture<'static, GlobalResult<String>> {
            let id = task.id.clone();
            let desc = task.description.clone();
            Box::pin(async move {
                let out = match id.as_str() {
                    "growth_team"  => "LOWER_PRICE ($29/seat/mo): competitors at $49-79, price-sensitivity survey shows 68% of prospects drop off above $35, PLG motion needs low friction, land-and-expand works better with volume".to_string(),
                    "revenue_team" => "HIGHER_PRICE ($79/seat/mo): enterprise ACV increases 2.4x, support load per dollar drops, positions us away from SMB churn risk, comp analysis shows room up to $89 for feature set".to_string(),
                    "ceo" => {
                        let has_context = desc.contains("Debate Arguments");
                        format!(
                            "pricing_decision: {} — Launch at $49/seat/mo. Free tier for ≤5 seats (PLG motion). Enterprise custom pricing >50 seats. Revisit after 6 months of ARR data.",
                            if has_context { "both arguments reviewed." } else { "no context." }
                        )
                    }
                    _ => "done".to_string(),
                };
                Ok(out.to_string())
            })
        });

    let summary = DebateScheduler::new().execute(&mut dag, executor).await?;

    println!("Teams (ran in parallel):");
    for r in &summary.results {
        if r.task_id == "ceo" { continue; }
        let status = if r.outcome.is_success() { "OK" } else { "FAIL" };
        println!("  [{}] {} -> {}", status, r.task_id, r.outcome.output().unwrap_or("(no output)"));
    }

    if let Some(r) = summary.results.iter().find(|r| r.task_id == "ceo") {
        let status = if r.outcome.is_success() { "OK" } else { "FAIL" };
        println!("\nCEO decision (receives both arguments injected):\n  [{}] {} -> {}",
            status, r.task_id, r.outcome.output().unwrap_or("(no output)"));
    }

    println!("\nsucceeded={} failed={} total_time={:?}",
        summary.succeeded, summary.failed, summary.total_wall_time);

    Ok(())
}
