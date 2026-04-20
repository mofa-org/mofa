//! Consensus: Support Ticket Topic Classification
//!
//! Scenario: Three classifier agents independently label an inbound
//! support ticket. The ticket router reads all labels and picks the
//! majority category to determine the correct queue.
//!
//! Run: RUST_LOG=info cargo run -p swarm_coordination_patterns --bin consensus_topic_classification

use std::sync::Arc;

use anyhow::Result;
use futures::future::BoxFuture;
use mofa_foundation::swarm::{
    ConsensusScheduler, SubtaskDAG, SubtaskExecutorFn, SwarmScheduler, SwarmSubtask,
};
use mofa_kernel::agent::types::error::GlobalResult;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    println!("=== Consensus: Support Ticket Topic Classification ===");
    println!("DAG: classifier_bert, classifier_fasttext, classifier_rules (voters) -> ticket_router (aggregator)\n");
    println!("Ticket: 'My invoice shows a charge I didn't authorise from last month'\n");

    let mut dag = SubtaskDAG::new("ticket-classification");

    let bert      = dag.add_task(SwarmSubtask::new("classifier_bert",     "BERT-based classifier: label the support ticket topic"));
    let fasttext  = dag.add_task(SwarmSubtask::new("classifier_fasttext", "FastText classifier: label the support ticket topic"));
    let rules_clf = dag.add_task(SwarmSubtask::new("classifier_rules",    "Keyword rules classifier: label the support ticket topic"));
    let router    = dag.add_task(SwarmSubtask::new("ticket_router",       "Apply majority label to route ticket to the correct support queue"));

    dag.add_dependency(bert, router)?;
    dag.add_dependency(fasttext, router)?;
    dag.add_dependency(rules_clf, router)?;

    let executor: SubtaskExecutorFn =
        Arc::new(move |_idx, task: SwarmSubtask| -> BoxFuture<'static, GlobalResult<String>> {
            let id = task.id.clone();
            let desc = task.description.clone();
            Box::pin(async move {
                let out = match id.as_str() {
                    "classifier_bert"     => "billing".to_string(),
                    "classifier_fasttext" => "billing".to_string(),
                    "classifier_rules"    => "dispute".to_string(),
                    "ticket_router" => {
                        let has_context = desc.contains("Voter Outputs");
                        format!(
                            "routing_decision: BILLING — {} 2/3 classifiers agree on 'billing' (majority). Ticket TKT-00492 assigned to billing-disputes queue. SLA: 4h first response.",
                            if has_context { "all classifier outputs reviewed." } else { "no classifier data." }
                        )
                    }
                    _ => "done".to_string(),
                };
                Ok(out.to_string())
            })
        });

    let summary = ConsensusScheduler::new().execute(&mut dag, executor).await?;

    println!("Classifiers (ran in parallel):");
    for r in &summary.results {
        if r.task_id == "ticket_router" { continue; }
        let status = if r.outcome.is_success() { "OK" } else { "FAIL" };
        println!("  [{}] {} -> {}", status, r.task_id, r.outcome.output().unwrap_or("(no output)"));
    }

    if let Some(r) = summary.results.iter().find(|r| r.task_id == "ticket_router") {
        let status = if r.outcome.is_success() { "OK" } else { "FAIL" };
        println!("\nTicket router (receives all labels injected):\n  [{}] {} -> {}",
            status, r.task_id, r.outcome.output().unwrap_or("(no output)"));
    }

    println!("\nsucceeded={} failed={} total_time={:?}",
        summary.succeeded, summary.failed, summary.total_wall_time);

    Ok(())
}
