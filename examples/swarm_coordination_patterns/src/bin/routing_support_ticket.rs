use std::sync::Arc;

use anyhow::Result;
use futures::future::BoxFuture;
use mofa_foundation::swarm::{
    RoutingScheduler, SubtaskDAG, SubtaskExecutorFn, SwarmScheduler, SwarmSubtask,
};
use mofa_kernel::agent::types::error::GlobalResult;

fn ticket_executor() -> SubtaskExecutorFn {
    Arc::new(move |_idx, task: SwarmSubtask| -> BoxFuture<'static, GlobalResult<String>> {
        let id = task.id.clone();
        let desc = task.description.clone();
        Box::pin(async move {
            let output = match id.as_str() {
                "ticket_classifier" => {
                    "billing issue with payment: customer charged twice for subscription renewal".to_string()
                }
                "billing_agent" => {
                    let routed = if desc.contains("Router Output") { "with router context" } else { "no context" };
                    format!("billing_resolved: duplicate charge identified and refund initiated ({})", routed)
                }
                "technical_agent" => {
                    "technical_resolved: bug triaged and assigned to engineering team".to_string()
                }
                "general_agent" => {
                    "general_resolved: ticket acknowledged, routed to appropriate team".to_string()
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

    let mut dag = SubtaskDAG::new("support-ticket-routing");

    let classifier = dag.add_task(SwarmSubtask::new(
        "ticket_classifier",
        "Classify incoming support ticket by reading subject and body",
    ));

    let mut billing = SwarmSubtask::new("billing_agent", "Handle billing disputes and payment issues");
    billing.required_capabilities = vec!["billing".into()];
    let billing_idx = dag.add_task(billing);

    let mut technical = SwarmSubtask::new("technical_agent", "Handle technical bugs and integration issues");
    technical.required_capabilities = vec!["technical".into(), "bug".into()];
    let technical_idx = dag.add_task(technical);

    let mut general = SwarmSubtask::new("general_agent", "Handle general inquiries and account questions");
    general.required_capabilities = vec!["general".into()];
    let general_idx = dag.add_task(general);

    dag.add_dependency(classifier, billing_idx)?;
    dag.add_dependency(classifier, technical_idx)?;
    dag.add_dependency(classifier, general_idx)?;

    println!("=== Routing: Customer Support Ticket Dispatch ===");
    println!("Specialists: billing_agent [billing], technical_agent [technical, bug], general_agent [general]");
    println!("Router output will contain 'billing' -> billing_agent selected\n");

    let summary = RoutingScheduler::new()
        .execute(&mut dag, ticket_executor())
        .await?;

    let router_result = summary.results.iter().find(|r| r.task_id == "ticket_classifier");
    if let Some(r) = router_result {
        println!("Router output:");
        println!("  ticket_classifier -> {}", r.outcome.output().unwrap_or("(no output)"));
    }

    println!("\nSpecialist routing decisions:");
    for r in &summary.results {
        if r.task_id == "ticket_classifier" {
            continue;
        }
        use mofa_foundation::swarm::TaskOutcome;
        let status = match &r.outcome {
            TaskOutcome::Success(_) => "SELECTED",
            TaskOutcome::Skipped(_) => "SKIPPED ",
            TaskOutcome::Failure(_) => "FAILED  ",
        };
        let detail = match &r.outcome {
            TaskOutcome::Success(s) => s.as_str(),
            TaskOutcome::Skipped(s) => s.as_str(),
            TaskOutcome::Failure(s) => s.as_str(),
        };
        println!("  [{}] {} -> {}", status, r.task_id, detail);
    }

    println!("\nSummary: succeeded={} skipped={} failed={} total_wall_time={:?}",
        summary.succeeded, summary.skipped, summary.failed, summary.total_wall_time);

    Ok(())
}
