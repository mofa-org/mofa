//! Sequential: User Onboarding Flow
//!
//! Scenario: New user sign-up pipeline — verify email → create profile
//! → send welcome email → assign to team. Each step depends on the previous.
//!
//! Run: RUST_LOG=info cargo run -p swarm_coordination_patterns --bin sequential_onboarding_flow

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

    println!("=== Sequential: User Onboarding Flow ===");
    println!("DAG: verify_email -> create_profile -> send_welcome -> assign_team\n");

    let mut dag = SubtaskDAG::new("onboarding");

    let verify  = dag.add_task(SwarmSubtask::new("verify_email",   "Confirm the user's email address is valid and reachable"));
    let profile = dag.add_task(SwarmSubtask::new("create_profile", "Create the user account and default workspace settings"));
    let welcome = dag.add_task(SwarmSubtask::new("send_welcome",   "Send the welcome email with getting-started links"));
    let assign  = dag.add_task(SwarmSubtask::new("assign_team",    "Add the user to their assigned team and notify the team lead"));

    dag.add_dependency(verify, profile)?;
    dag.add_dependency(profile, welcome)?;
    dag.add_dependency(welcome, assign)?;

    let executor: SubtaskExecutorFn =
        Arc::new(move |_idx, task: SwarmSubtask| -> BoxFuture<'static, GlobalResult<String>> {
            let id = task.id.clone();
            Box::pin(async move {
                let out = match id.as_str() {
                    "verify_email"   => "email_verified: nityam@example.com — MX record confirmed, no bounce history",
                    "create_profile" => "profile_created: user_id=usr_9f2a, workspace=ws_default, tier=free",
                    "send_welcome"   => "email_sent: message_id=msg_3c81, delivered at 14:02 UTC",
                    "assign_team"    => "team_assigned: team=backend-platform, lead notified via Slack #team-updates",
                    _                => "done",
                };
                Ok(out.to_string())
            })
        });

    let summary = SequentialScheduler::new().execute(&mut dag, executor).await?;

    println!("Onboarding steps:");
    for r in &summary.results {
        let status = if r.outcome.is_success() { "OK" } else { "FAIL" };
        println!("  [{}] {} -> {}", status, r.task_id, r.outcome.output().unwrap_or("(no output)"));
    }

    println!("\nsucceeded={} failed={} total_time={:?}",
        summary.succeeded, summary.failed, summary.total_wall_time);

    Ok(())
}
