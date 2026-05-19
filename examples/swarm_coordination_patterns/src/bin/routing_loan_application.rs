//! Routing: Loan Application Dispatch
//!
//! Scenario: A classifier reads the loan application and determines its
//! type (personal, auto, or mortgage). The correct underwriter handles it;
//! the other two are skipped.
//!
//! Run: RUST_LOG=info cargo run -p swarm_coordination_patterns --bin routing_loan_application

use std::sync::Arc;

use anyhow::Result;
use futures::future::BoxFuture;
use mofa_foundation::swarm::{
    RoutingScheduler, SubtaskDAG, SubtaskExecutorFn, SwarmScheduler, SwarmSubtask,
};
use mofa_kernel::agent::types::error::GlobalResult;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    println!("=== Routing: Loan Application Dispatch ===");
    println!("Application: $380,000 fixed-rate home purchase, 20% down, 30yr term");
    println!("Underwriters: personal [personal], auto [auto], mortgage [mortgage]\n");

    let mut dag = SubtaskDAG::new("loan-routing");

    let classifier = dag.add_task(SwarmSubtask::new("loan_classifier", "Read the application and identify the loan type"));

    let mut personal = SwarmSubtask::new("personal_underwriter", "Underwrite the personal loan application");
    personal.required_capabilities = vec!["personal".into()];
    let idx_personal = dag.add_task(personal);

    let mut auto = SwarmSubtask::new("auto_underwriter", "Underwrite the auto loan application");
    auto.required_capabilities = vec!["auto".into()];
    let idx_auto = dag.add_task(auto);

    let mut mortgage = SwarmSubtask::new("mortgage_underwriter", "Underwrite the mortgage application");
    mortgage.required_capabilities = vec!["mortgage".into()];
    let idx_mortgage = dag.add_task(mortgage);

    dag.add_dependency(classifier, idx_personal)?;
    dag.add_dependency(classifier, idx_auto)?;
    dag.add_dependency(classifier, idx_mortgage)?;

    let executor: SubtaskExecutorFn =
        Arc::new(move |_idx, task: SwarmSubtask| -> BoxFuture<'static, GlobalResult<String>> {
            let id = task.id.clone();
            Box::pin(async move {
                let out = match id.as_str() {
                    "loan_classifier"      => "loan type: mortgage — $380k home purchase, LTV 80%, 30yr fixed, applicant credit score 748",
                    "mortgage_underwriter" => "mortgage_approved: LTV 80% within guidelines, DTI 28% (below 43% threshold), rate locked at 6.875% 30yr fixed, closing in 21 days",
                    "personal_underwriter" => "not applicable for this application type",
                    "auto_underwriter"     => "not applicable for this application type",
                    _                      => "done",
                };
                Ok(out.to_string())
            })
        });

    let summary = RoutingScheduler::new().execute(&mut dag, executor).await?;

    println!("Classifier output:");
    if let Some(r) = summary.results.iter().find(|r| r.task_id == "loan_classifier") {
        println!("  [OK] loan_classifier -> {}", r.outcome.output().unwrap_or(""));
    }

    println!("\nUnderwriter routing:");
    for r in &summary.results {
        if r.task_id == "loan_classifier" { continue; }
        let label = match &r.outcome {
            mofa_foundation::swarm::TaskOutcome::Success(s) => format!("[SELECTED] {}", s),
            mofa_foundation::swarm::TaskOutcome::Skipped(_) => "[SKIPPED]".into(),
            mofa_foundation::swarm::TaskOutcome::Failure(e) => format!("[FAIL] {}", e),
        };
        println!("  {} -> {}", r.task_id, label);
    }

    println!("\nsucceeded={} skipped={} total_time={:?}",
        summary.succeeded, summary.skipped, summary.total_wall_time);

    Ok(())
}
