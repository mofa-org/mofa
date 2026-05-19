use std::sync::Arc;

use anyhow::Result;
use futures::future::BoxFuture;
use mofa_foundation::swarm::{
    MapReduceScheduler, SubtaskDAG, SubtaskExecutorFn, SwarmScheduler, SwarmSubtask,
};
use mofa_kernel::agent::types::error::GlobalResult;

fn summary_executor() -> SubtaskExecutorFn {
    Arc::new(move |_idx, task: SwarmSubtask| -> BoxFuture<'static, GlobalResult<String>> {
        let id = task.id.clone();
        let desc = task.description.clone();
        Box::pin(async move {
            let output = match id.as_str() {
                "section_1" => "section_1_summary: Introduction covers transformer architecture motivation and prior work on attention mechanisms".to_string(),
                "section_2" => "section_2_summary: Methodology details multi-head attention, positional encoding, and encoder-decoder stack".to_string(),
                "section_3" => "section_3_summary: Experiments show BLEU score improvements of 2.0 over previous SOTA on WMT 2014 En-De".to_string(),
                "section_4" => "section_4_summary: Conclusion argues self-attention generalizes beyond NLP to vision and reinforcement learning".to_string(),
                "final_summary" => {
                    format!(
                        "final_paper_summary: Paper presents the Transformer model ({}). All 4 sections integrated.",
                        if desc.contains("Map Phase Outputs") { "context received from mappers" } else { "no context" }
                    )
                }
                _ => format!("{}: summarized", id),
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

    let mut dag = SubtaskDAG::new("document-summarization");

    let s1 = dag.add_task(SwarmSubtask::new("section_1", "Summarize Introduction section"));
    let s2 = dag.add_task(SwarmSubtask::new("section_2", "Summarize Methodology section"));
    let s3 = dag.add_task(SwarmSubtask::new("section_3", "Summarize Experiments section"));
    let s4 = dag.add_task(SwarmSubtask::new("section_4", "Summarize Conclusion section"));
    let reducer = dag.add_task(SwarmSubtask::new("final_summary", "Merge all section summaries into paper abstract"));

    dag.add_dependency(s1, reducer)?;
    dag.add_dependency(s2, reducer)?;
    dag.add_dependency(s3, reducer)?;
    dag.add_dependency(s4, reducer)?;

    println!("=== MapReduce: Research Paper Summarization ===");
    println!("DAG: section_1, section_2, section_3, section_4 (mappers) -> final_summary (reducer)\n");

    let summary = MapReduceScheduler::new()
        .execute(&mut dag, summary_executor())
        .await?;

    let mappers: Vec<_> = summary.results.iter()
        .filter(|r| r.task_id != "final_summary")
        .collect();
    let reducer_result = summary.results.iter().find(|r| r.task_id == "final_summary");

    println!("Map phase (parallel section summarizers):");
    for r in &mappers {
        let status = if r.outcome.is_success() { "OK" } else { "FAIL" };
        println!("  [{}] {} -> {}", status, r.task_id, r.outcome.output().unwrap_or("(no output)"));
    }

    if let Some(r) = reducer_result {
        let status = if r.outcome.is_success() { "OK" } else { "FAIL" };
        println!("\nReduce phase (all mapper outputs injected into description):");
        println!("  [{}] {} -> {}", status, r.task_id, r.outcome.output().unwrap_or("(no output)"));
    }

    println!("\nSummary: succeeded={} failed={} total_wall_time={:?}",
        summary.succeeded, summary.failed, summary.total_wall_time);

    Ok(())
}
