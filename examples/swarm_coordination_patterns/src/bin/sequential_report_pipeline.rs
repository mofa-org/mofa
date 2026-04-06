use std::sync::Arc;

use anyhow::Result;
use futures::future::BoxFuture;
use mofa_foundation::swarm::{
    SequentialScheduler, SubtaskDAG, SubtaskExecutorFn, SwarmScheduler, SwarmSubtask,
};
use mofa_kernel::agent::types::error::GlobalResult;

fn stage_executor() -> SubtaskExecutorFn {
    Arc::new(move |_idx, task: SwarmSubtask| -> BoxFuture<'static, GlobalResult<String>> {
        let id = task.id.clone();
        Box::pin(async move {
            let output = match id.as_str() {
                "fetch_data" => "market_data_fetched: S&P 500 -2.3%, NASDAQ +1.1%, DJI -0.8%".to_string(),
                "analyze_trends" => "trend_analysis: bearish sentiment in tech, bullish in energy".to_string(),
                "write_draft" => "draft_written: 3-section report covering macro outlook, sector rotation, risk factors".to_string(),
                "proofread" => "proofread_complete: grammar corrected, citations verified, executive summary added".to_string(),
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

    let mut dag = SubtaskDAG::new("market-report-pipeline");

    let fetch = dag.add_task(SwarmSubtask::new("fetch_data", "Fetch live market data feeds"));
    let analyze = dag.add_task(SwarmSubtask::new("analyze_trends", "Identify market trends from raw data"));
    let draft = dag.add_task(SwarmSubtask::new("write_draft", "Write the first draft of the report"));
    let proof = dag.add_task(SwarmSubtask::new("proofread", "Proofread and finalize the report"));

    dag.add_dependency(fetch, analyze)?;
    dag.add_dependency(analyze, draft)?;
    dag.add_dependency(draft, proof)?;

    println!("=== Sequential: Market Report Generation ===");
    println!("DAG: fetch_data -> analyze_trends -> write_draft -> proofread\n");

    let summary = SequentialScheduler::new()
        .execute(&mut dag, stage_executor())
        .await?;

    println!("Execution order (sequential guarantee):");
    for result in &summary.results {
        let status = if result.outcome.is_success() { "OK" } else { "FAIL" };
        let output = result.outcome.output().unwrap_or("(no output)");
        println!("  [{}] {} -> {}", status, result.task_id, output);
    }

    println!("\nSummary: succeeded={} failed={} total_wall_time={:?}",
        summary.succeeded, summary.failed, summary.total_wall_time);

    Ok(())
}
