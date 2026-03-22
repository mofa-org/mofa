//! Consensus: Transaction Fraud Detection
//!
//! Scenario: Three ML models independently classify a payment transaction
//! as fraudulent or legitimate. A risk adjudicator reads all three verdicts
//! and applies majority rule to issue the final fraud flag.
//!
//! Run: RUST_LOG=info cargo run -p swarm_coordination_patterns --bin consensus_fraud_detection

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

    println!("=== Consensus: Transaction Fraud Detection ===");
    println!("DAG: model_xgb, model_lstm, model_rules (voters) -> risk_adjudicator (aggregator)\n");
    println!("Transaction: $4,200 card-not-present purchase, new device, overseas IP\n");

    let mut dag = SubtaskDAG::new("fraud-detection");

    let xgb   = dag.add_task(SwarmSubtask::new("model_xgb",   "XGBoost classifier: score this transaction for fraud probability"));
    let lstm  = dag.add_task(SwarmSubtask::new("model_lstm",  "LSTM sequence model: analyse spending pattern anomaly for fraud"));
    let rules = dag.add_task(SwarmSubtask::new("model_rules", "Rules engine: apply velocity and geo-mismatch fraud rules"));
    let adj   = dag.add_task(SwarmSubtask::new("risk_adjudicator", "Combine all model verdicts and issue final fraud flag"));

    dag.add_dependency(xgb, adj)?;
    dag.add_dependency(lstm, adj)?;
    dag.add_dependency(rules, adj)?;

    let executor: SubtaskExecutorFn =
        Arc::new(move |_idx, task: SwarmSubtask| -> BoxFuture<'static, GlobalResult<String>> {
            let id = task.id.clone();
            let desc = task.description.clone();
            Box::pin(async move {
                let out = match id.as_str() {
                    "model_xgb"   => "fraud".to_string(),
                    "model_lstm"  => "fraud".to_string(),
                    "model_rules" => "legitimate: geo-mismatch flagged but within 30-day travel window".to_string(),
                    "risk_adjudicator" => {
                        let has_context = desc.contains("Voter Outputs");
                        format!(
                            "fraud_verdict: FRAUD — {} 2/3 models flagged fraud (majority). Transaction blocked. Card step-up challenge sent to cardholder. Case ref: FRAUD-2024-8821.",
                            if has_context { "all model outputs reviewed." } else { "no model data." }
                        )
                    }
                    _ => "done".to_string(),
                };
                Ok(out.to_string())
            })
        });

    let summary = ConsensusScheduler::new().execute(&mut dag, executor).await?;

    println!("Models (ran in parallel):");
    for r in &summary.results {
        if r.task_id == "risk_adjudicator" { continue; }
        let status = if r.outcome.is_success() { "OK" } else { "FAIL" };
        println!("  [{}] {} -> {}", status, r.task_id, r.outcome.output().unwrap_or("(no output)"));
    }

    if let Some(r) = summary.results.iter().find(|r| r.task_id == "risk_adjudicator") {
        let status = if r.outcome.is_success() { "OK" } else { "FAIL" };
        println!("\nRisk adjudicator (receives all votes injected):\n  [{}] {} -> {}",
            status, r.task_id, r.outcome.output().unwrap_or("(no output)"));
    }

    println!("\nsucceeded={} failed={} total_time={:?}",
        summary.succeeded, summary.failed, summary.total_wall_time);

    Ok(())
}
