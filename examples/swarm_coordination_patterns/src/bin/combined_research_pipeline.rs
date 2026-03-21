use std::sync::Arc;

use anyhow::Result;
use futures::future::BoxFuture;
use mofa_foundation::swarm::{
    DebateScheduler, MapReduceScheduler, SubtaskDAG, SubtaskExecutorFn, SwarmScheduler,
    SwarmSubtask,
};
use mofa_kernel::agent::types::error::GlobalResult;

fn phase1_executor() -> SubtaskExecutorFn {
    Arc::new(move |_idx, task: SwarmSubtask| -> BoxFuture<'static, GlobalResult<String>> {
        let id = task.id.clone();
        let desc = task.description.clone();
        Box::pin(async move {
            let output = match id.as_str() {
                "arxiv_agent" => {
                    "arxiv_findings: 47 papers on LLM reasoning published Q4 2024; \
                     chain-of-thought variants dominate, 3 papers challenge scaling assumptions".to_string()
                }
                "semantic_scholar_agent" => {
                    "semantic_findings: citation graph shows 3 seminal papers with >1000 cites; \
                     GPT-4 technical report is most referenced in reasoning benchmarks".to_string()
                }
                "web_crawler_agent" => {
                    "web_findings: industry blogs report practitioners prefer few-shot prompting \
                     over fine-tuning for reasoning tasks; tool use is emerging pattern".to_string()
                }
                "research_consolidator" => {
                    let has_map_outputs = desc.contains("Map Phase Outputs");
                    format!(
                        "consolidated_corpus: unified knowledge base from 3 sources covering \
                         47 papers, citation graph, and practitioner reports. \
                         Ready for debate analysis. Map outputs integrated: {}",
                        has_map_outputs
                    )
                }
                _ => format!("{}: processed", id),
            };
            Ok(output)
        })
    })
}

fn phase2_executor(phase1_output: String) -> SubtaskExecutorFn {
    Arc::new(move |_idx, task: SwarmSubtask| -> BoxFuture<'static, GlobalResult<String>> {
        let id = task.id.clone();
        let desc = task.description.clone();
        let corpus = phase1_output.clone();
        Box::pin(async move {
            let output = match id.as_str() {
                "scaling_optimist" => {
                    format!(
                        "optimist_position: Evidence from corpus ({} chars) supports continued \
                         scaling: benchmark improvements correlate with parameter count, \
                         reasoning emerges at sufficient scale",
                        corpus.len()
                    )
                }
                "efficiency_realist" => {
                    format!(
                        "realist_position: Corpus ({} chars) shows diminishing returns above 70B params; \
                         mixture-of-experts and retrieval augmentation outperform raw scaling \
                         per FLOP on reasoning tasks",
                        corpus.len()
                    )
                }
                "synthesis_lead" => {
                    let has_debate = desc.contains("Debate Arguments");
                    format!(
                        "synthesis_conclusion: Both perspectives valid. Recommendation: scale to \
                         sweet spot (~30-70B), then invest in retrieval and tool-use infrastructure. \
                         Debate context received: {}",
                        has_debate
                    )
                }
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

    println!("=== Combined Pipeline: MapReduce + Debate for Research Synthesis ===\n");

    println!("--- Phase 1: MapReduce - Gather and consolidate research data ---");

    let mut phase1_dag = SubtaskDAG::new("research-gathering");

    let arxiv = phase1_dag.add_task(SwarmSubtask::new(
        "arxiv_agent",
        "Query arXiv for recent LLM reasoning papers",
    ));
    let semantic = phase1_dag.add_task(SwarmSubtask::new(
        "semantic_scholar_agent",
        "Query Semantic Scholar for citation graph analysis",
    ));
    let web = phase1_dag.add_task(SwarmSubtask::new(
        "web_crawler_agent",
        "Crawl tech blogs and forums for practitioner insights",
    ));
    let consolidator = phase1_dag.add_task(SwarmSubtask::new(
        "research_consolidator",
        "Merge all source findings into unified research corpus",
    ));

    phase1_dag.add_dependency(arxiv, consolidator)?;
    phase1_dag.add_dependency(semantic, consolidator)?;
    phase1_dag.add_dependency(web, consolidator)?;

    let phase1_summary = MapReduceScheduler::new()
        .execute(&mut phase1_dag, phase1_executor())
        .await?;

    println!("Data gathering agents:");
    for r in &phase1_summary.results {
        if r.task_id == "research_consolidator" {
            continue;
        }
        let status = if r.outcome.is_success() { "OK" } else { "FAIL" };
        println!("  [{}] {} -> {}", status, r.task_id, r.outcome.output().unwrap_or("(no output)"));
    }

    let consolidated_output = phase1_summary.results.iter()
        .find(|r| r.task_id == "research_consolidator")
        .and_then(|r| r.outcome.output())
        .unwrap_or("empty corpus")
        .to_string();

    println!("\nConsolidated corpus:");
    println!("  research_consolidator -> {}", consolidated_output);
    println!("\nPhase 1 complete: succeeded={} failed={}", phase1_summary.succeeded, phase1_summary.failed);

    println!("\n--- Phase 2: Debate - Analysts argue over the consolidated findings ---");

    let mut phase2_dag = SubtaskDAG::new("findings-debate");

    let optimist = phase2_dag.add_task(SwarmSubtask::new(
        "scaling_optimist",
        "Argue that scaling LLMs remains the primary path to better reasoning",
    ));
    let realist = phase2_dag.add_task(SwarmSubtask::new(
        "efficiency_realist",
        "Argue that architectural improvements outperform raw scaling",
    ));
    let synthesis = phase2_dag.add_task(SwarmSubtask::new(
        "synthesis_lead",
        "Synthesize the debate and issue actionable research recommendation",
    ));

    phase2_dag.add_dependency(optimist, synthesis)?;
    phase2_dag.add_dependency(realist, synthesis)?;

    let phase2_summary = DebateScheduler::new()
        .execute(&mut phase2_dag, phase2_executor(consolidated_output))
        .await?;

    println!("Analysts (debate based on Phase 1 corpus):");
    for r in &phase2_summary.results {
        if r.task_id == "synthesis_lead" {
            continue;
        }
        let status = if r.outcome.is_success() { "OK" } else { "FAIL" };
        println!("  [{}] {} ->\n    {}", status, r.task_id, r.outcome.output().unwrap_or("(no output)"));
    }

    let synthesis_result = phase2_summary.results.iter()
        .find(|r| r.task_id == "synthesis_lead");
    if let Some(r) = synthesis_result {
        let status = if r.outcome.is_success() { "OK" } else { "FAIL" };
        println!("\nSynthesis lead (receives debate arguments injected into description):");
        println!("  [{}] {} ->\n    {}", status, r.task_id, r.outcome.output().unwrap_or("(no output)"));
    }

    println!("\nPhase 2 complete: succeeded={} failed={}", phase2_summary.succeeded, phase2_summary.failed);

    println!("\n=== Combined Pipeline Complete ===");
    println!("Phase 1 (MapReduce): {} tasks", phase1_summary.total_tasks);
    println!("Phase 2 (Debate):    {} tasks", phase2_summary.total_tasks);
    println!("Total wall time:     Phase1={:?}, Phase2={:?}",
        phase1_summary.total_wall_time, phase2_summary.total_wall_time);

    Ok(())
}
