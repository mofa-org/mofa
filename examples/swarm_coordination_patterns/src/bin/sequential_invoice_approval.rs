//! Sequential: Invoice Approval Workflow
//!
//! Scenario: Finance pipeline — draft invoice → manager review
//! → finance review → send to client. Strict ordering enforced.
//!
//! Run: RUST_LOG=info cargo run -p swarm_coordination_patterns --bin sequential_invoice_approval

use std::sync::Arc;

use anyhow::Result;
use futures::future::BoxFuture;
use mofa_foundation::swarm::{
    SequentialScheduler, SubtaskDAG, SubtaskExecutorFn, SwarmScheduler, SwarmSubtask,
};
use mofa_kernel::agent::types::error::GlobalResult;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    println!("=== Sequential: Invoice Approval Workflow ===");
    println!("DAG: draft_invoice -> manager_review -> finance_review -> send_to_client\n");

    let mut dag = SubtaskDAG::new("invoice-approval");

    let draft   = dag.add_task(SwarmSubtask::new("draft_invoice",   "Generate invoice from time-tracking data for client Acme Corp"));
    let manager = dag.add_task(SwarmSubtask::new("manager_review",  "Manager verifies hours and line items before escalation"));
    let finance = dag.add_task(SwarmSubtask::new("finance_review",  "Finance team checks tax codes and payment terms"));
    let send    = dag.add_task(SwarmSubtask::new("send_to_client",  "Deliver approved invoice via email and mark as outstanding in ERP"));

    dag.add_dependency(draft, manager)?;
    dag.add_dependency(manager, finance)?;
    dag.add_dependency(finance, send)?;

    let executor: SubtaskExecutorFn =
        Arc::new(move |_idx, task: SwarmSubtask| -> BoxFuture<'static, GlobalResult<String>> {
            let id = task.id.clone();
            Box::pin(async move {
                let out = match id.as_str() {
                    "draft_invoice"  => "draft_ok: INV-2024-0381, total $12,450.00 (160 hrs @ $75 + $450 expenses)",
                    "manager_review" => "manager_approved: hours confirmed, note added — exclude March 15 public holiday (8 hrs removed, revised $11,850.00)",
                    "finance_review" => "finance_approved: tax_code=B2B-EXEMPT, net_30 terms, VAT N/A for US client",
                    "send_to_client" => "sent_ok: delivered to billing@acme.com at 09:14 UTC, ERP ref=INV-2024-0381, status=OUTSTANDING",
                    _                => "done",
                };
                Ok(out.to_string())
            })
        });

    let summary = SequentialScheduler::new().execute(&mut dag, executor).await?;

    println!("Approval stages:");
    for r in &summary.results {
        let status = if r.outcome.is_success() { "OK" } else { "FAIL" };
        println!("  [{}] {} -> {}", status, r.task_id, r.outcome.output().unwrap_or("(no output)"));
    }

    println!("\nsucceeded={} failed={} total_time={:?}",
        summary.succeeded, summary.failed, summary.total_wall_time);

    Ok(())
}
