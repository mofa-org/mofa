use std::sync::Arc;
use std::time::Duration;

use futures::future::BoxFuture;
use mofa_foundation::swarm::{
    ParallelScheduler, RiskLevel, SubtaskDAG, SwarmScheduler, SwarmSubtask,
};
use mofa_kernel::agent::types::error::GlobalResult;
use mofa_testing::SwarmRunArtifact;

#[tokio::main]
async fn main() -> GlobalResult<()> {
    let mut dag = SubtaskDAG::new("incident-response");

    let triage = dag.add_task(
        SwarmSubtask::new("triage", "Triage incoming incident")
            .with_capabilities(vec!["triage".into()]),
    );
    let investigate = dag.add_task(
        SwarmSubtask::new("investigate", "Investigate impact")
            .with_capabilities(vec!["analysis".into()]),
    );
    let remediate = dag.add_task(
        SwarmSubtask::new("remediate", "Apply remediation")
            .with_capabilities(vec!["ops".into()])
            .with_risk_level(RiskLevel::High),
    );
    let report = dag.add_task(
        SwarmSubtask::new("report", "Publish summary")
            .with_capabilities(vec!["reporting".into()]),
    );

    dag.add_dependency(triage, investigate)?;
    dag.add_dependency(investigate, remediate)?;
    dag.add_dependency(remediate, report)?;

    dag.assign_agent(triage, "router");
    dag.assign_agent(investigate, "analyst");
    dag.assign_agent(remediate, "operator");
    dag.assign_agent(report, "communicator");

    let executor = Arc::new(|_idx, task: SwarmSubtask| -> BoxFuture<'static, _> {
        Box::pin(async move {
            tokio::time::sleep(Duration::from_millis(15)).await;
            Ok(format!(
                "{} handled by {}",
                task.id,
                task.assigned_agent.unwrap_or_else(|| "unassigned".into())
            ))
        })
    });

    let scheduler = ParallelScheduler::new();
    let summary = scheduler.execute(&mut dag, executor).await?;
    let artifact = SwarmRunArtifact::from_scheduler_run(&dag, &summary);

    println!("{}", artifact.to_markdown());
    println!("{}", artifact.to_json());

    Ok(())
}
