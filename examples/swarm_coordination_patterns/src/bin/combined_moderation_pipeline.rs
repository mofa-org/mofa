//! Combined example: Routing + Supervision
//!
//! Scenario: Content moderation pipeline
//!
//! Phase 1 (Routing): A classifier reads the submission and routes it to
//!   the correct specialist — text, image, or video review agent.
//!
//! Phase 2 (Supervision): The chosen specialist runs alongside a policy
//!   checker. If either fails, a compliance officer (supervisor) reviews
//!   all outcomes and issues a final ruling.
//!
//! Run: RUST_LOG=info cargo run -p swarm_coordination_patterns --bin combined_moderation_pipeline

use std::sync::Arc;

use anyhow::Result;
use futures::future::BoxFuture;
use mofa_foundation::swarm::{
    RoutingScheduler, SubtaskDAG, SubtaskExecutorFn, SupervisionScheduler, SwarmScheduler,
    SwarmSubtask,
};
use mofa_kernel::agent::types::error::GlobalResult;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    println!("=== Combined Pipeline: Routing + Supervision ===");
    println!("Scenario: Content moderation for a user submission\n");

    let specialist_output = run_phase1_routing().await?;
    run_phase2_supervision(specialist_output).await?;

    println!("\n=== Pipeline Complete ===");
    Ok(())
}

async fn run_phase1_routing() -> Result<String> {
    println!("--- Phase 1: Routing - classify and dispatch submission ---");

    let mut dag = SubtaskDAG::new("content-routing");

    let router = dag.add_task(SwarmSubtask::new(
        "content_classifier",
        "Read the submission and identify its media type",
    ));

    let mut text_agent = SwarmSubtask::new("text_reviewer", "Review text for policy violations");
    text_agent.required_capabilities = vec!["text".into()];
    let idx_text = dag.add_task(text_agent);

    let mut image_agent = SwarmSubtask::new("image_reviewer", "Review image for policy violations");
    image_agent.required_capabilities = vec!["image".into()];
    let idx_image = dag.add_task(image_agent);

    let mut video_agent = SwarmSubtask::new("video_reviewer", "Review video for policy violations");
    video_agent.required_capabilities = vec!["video".into()];
    let idx_video = dag.add_task(video_agent);

    dag.add_dependency(router, idx_text)?;
    dag.add_dependency(router, idx_image)?;
    dag.add_dependency(router, idx_video)?;

    let executor: SubtaskExecutorFn =
        Arc::new(move |_idx, task: SwarmSubtask| -> BoxFuture<'static, GlobalResult<String>> {
            let id = task.id.clone();
            Box::pin(async move {
                match id.as_str() {
                    "content_classifier" => Ok(
                        "submission type: text — blog post flagged for potential misinformation".into(),
                    ),
                    "text_reviewer" => Ok(
                        "text_review_complete: 3 claims require fact-check, 0 hate-speech markers, recommend human review".into(),
                    ),
                    "image_reviewer" => Ok("image_review_complete: no violations".into()),
                    "video_reviewer" => Ok("video_review_complete: no violations".into()),
                    _ => Ok(format!("{}: done", id)),
                }
            })
        });

    let summary = RoutingScheduler::new()
        .execute(&mut dag, executor)
        .await?;

    println!("Classifier output:");
    let router_result = summary.results.iter().find(|r| r.task_id == "content_classifier");
    if let Some(r) = router_result {
        println!("  [OK] content_classifier -> {}", r.outcome.output().unwrap_or(""));
    }

    println!("\nSpecialist routing:");
    for r in &summary.results {
        if r.task_id == "content_classifier" {
            continue;
        }
        let label = match &r.outcome {
            mofa_foundation::swarm::TaskOutcome::Success(s) => format!("[SELECTED] {}", s),
            mofa_foundation::swarm::TaskOutcome::Skipped(_) => "[SKIPPED]".into(),
            mofa_foundation::swarm::TaskOutcome::Failure(e) => format!("[FAIL] {}", e),
        };
        println!("  {} -> {}", r.task_id, label);
    }

    println!(
        "\nPhase 1 complete: succeeded={} skipped={}",
        summary.succeeded, summary.skipped
    );

    let specialist_output = summary
        .results
        .iter()
        .find(|r| r.outcome.is_success() && r.task_id != "content_classifier")
        .and_then(|r| r.outcome.output())
        .unwrap_or("no specialist output")
        .to_string();

    Ok(specialist_output)
}

async fn run_phase2_supervision(specialist_context: String) -> Result<()> {
    println!("\n--- Phase 2: Supervision - policy check with fault recovery ---");
    println!("Specialist finding fed into supervision phase: {}\n", specialist_context);

    let mut dag = SubtaskDAG::new("moderation-supervision");

    let w1 = dag.add_task(SwarmSubtask::new(
        "fact_checker",
        "Verify the 3 flagged claims against trusted sources",
    ));
    let w2 = dag.add_task(SwarmSubtask::new(
        "policy_checker",
        "Apply community guidelines rulebook to the submission",
    ));
    let supervisor = dag.add_task(SwarmSubtask::new(
        "compliance_officer",
        "Review all findings and issue final moderation ruling",
    ));

    dag.add_dependency(w1, supervisor)?;
    dag.add_dependency(w2, supervisor)?;

    let executor: SubtaskExecutorFn =
        Arc::new(move |_idx, task: SwarmSubtask| -> BoxFuture<'static, GlobalResult<String>> {
            let id = task.id.clone();
            Box::pin(async move {
                match id.as_str() {
                    "fact_checker" => Err(mofa_kernel::agent::types::error::GlobalError::runtime(
                        "fact_check_unavailable: external knowledge base API timeout",
                    )),
                    "policy_checker" => Ok(
                        "policy_check_passed: no explicit guideline violations detected".into(),
                    ),
                    "compliance_officer" => {
                        Ok("ruling: send_to_human_review — fact-check service unavailable, cannot auto-approve".into())
                    }
                    _ => Ok(format!("{}: done", id)),
                }
            })
        });

    let summary = SupervisionScheduler::new()
        .execute(&mut dag, executor)
        .await?;

    println!("Worker outcomes:");
    for r in &summary.results {
        if r.task_id == "compliance_officer" {
            continue;
        }
        let label = if r.outcome.is_success() { "SUCCESS" } else { "FAILED " };
        let detail = r
            .outcome
            .output()
            .unwrap_or_else(|| match &r.outcome {
                mofa_foundation::swarm::TaskOutcome::Failure(e) => e.as_str(),
                _ => "",
            });
        println!("  [{}] {} -> {}", label, r.task_id, detail);
    }

    let ruling = summary
        .results
        .iter()
        .find(|r| r.task_id == "compliance_officer");
    if let Some(r) = ruling {
        println!(
            "\nCompliance officer ruling (always runs, even with partial failure):\n  [OK] {}",
            r.outcome.output().unwrap_or("(no output)")
        );
    }

    println!(
        "\nPhase 2 complete: succeeded={} failed={}",
        summary.succeeded, summary.failed
    );

    Ok(())
}
