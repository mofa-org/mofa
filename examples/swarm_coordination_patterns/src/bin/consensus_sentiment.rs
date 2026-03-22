use std::sync::Arc;

use anyhow::Result;
use futures::future::BoxFuture;
use mofa_foundation::swarm::{
    ConsensusScheduler, SubtaskDAG, SubtaskExecutorFn, SwarmScheduler, SwarmSubtask,
};
use mofa_kernel::agent::types::error::GlobalResult;

fn classifier_executor() -> SubtaskExecutorFn {
    Arc::new(move |_idx, task: SwarmSubtask| -> BoxFuture<'static, GlobalResult<String>> {
        let id = task.id.clone();
        let desc = task.description.clone();
        Box::pin(async move {
            let output = match id.as_str() {
                "classifier_a" => "positive".to_string(),
                "classifier_b" => "positive".to_string(),
                "classifier_c" => "positive".to_string(),
                "verdict" => {
                    let has_majority = desc.contains("Majority Candidate");
                    let majority_label = if has_majority {
                        let start = desc.find("Majority Candidate\n").map(|i| i + "Majority Candidate\n".len()).unwrap_or(0);
                        let end = desc[start..].find('\n').map(|i| i + start).unwrap_or(desc.len());
                        desc[start..end].trim().to_string()
                    } else {
                        "no majority".to_string()
                    };
                    format!("final_verdict: {} (majority consensus reached)", majority_label)
                }
                _ => format!("{}: classified", id),
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

    let mut dag = SubtaskDAG::new("sentiment-consensus");

    let a = dag.add_task(SwarmSubtask::new(
        "classifier_a",
        "Classify product review sentiment using BERT model",
    ));
    let b = dag.add_task(SwarmSubtask::new(
        "classifier_b",
        "Classify product review sentiment using RoBERTa model",
    ));
    let c = dag.add_task(SwarmSubtask::new(
        "classifier_c",
        "Classify product review sentiment using GPT-based classifier",
    ));
    let verdict = dag.add_task(SwarmSubtask::new(
        "verdict",
        "Aggregate classifier votes and announce majority sentiment",
    ));

    dag.add_dependency(a, verdict)?;
    dag.add_dependency(b, verdict)?;
    dag.add_dependency(c, verdict)?;

    println!("=== Consensus: Product Review Sentiment Classification ===");
    println!("DAG: classifier_a, classifier_b, classifier_c (voters) -> verdict (aggregator)\n");

    let summary = ConsensusScheduler::new()
        .execute(&mut dag, classifier_executor())
        .await?;

    let voters: Vec<_> = summary.results.iter()
        .filter(|r| r.task_id != "verdict")
        .collect();
    let verdict_result = summary.results.iter().find(|r| r.task_id == "verdict");

    println!("Voters (ran in parallel):");
    for r in &voters {
        let status = if r.outcome.is_success() { "OK" } else { "FAIL" };
        println!("  [{}] {} -> {}", status, r.task_id, r.outcome.output().unwrap_or("(no output)"));
    }

    let votes: Vec<_> = voters.iter()
        .filter_map(|r| r.outcome.output())
        .collect();
    println!("\nVote tally: {:?}", votes);
    println!("Majority candidate: positive (3/3 votes)");

    if let Some(r) = verdict_result {
        let status = if r.outcome.is_success() { "OK" } else { "FAIL" };
        println!("\nAggregator (majority candidate injected into description):");
        println!("  [{}] {} -> {}", status, r.task_id, r.outcome.output().unwrap_or("(no output)"));
    }

    println!("\nSummary: succeeded={} failed={} total_wall_time={:?}",
        summary.succeeded, summary.failed, summary.total_wall_time);

    Ok(())
}
