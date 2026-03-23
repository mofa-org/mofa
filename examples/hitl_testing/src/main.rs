use mofa_kernel::hitl::{ExecutionStep, ExecutionTrace, ReviewContext, ReviewStatus};
use mofa_testing::{HitlTestHarness, ScriptedDecision};
use serde_json::json;
use std::collections::HashMap;
use std::time::Duration;

fn sample_context(task: &str) -> ReviewContext {
    // Build a review context so the example exercises the
    // same kernel HITL types used by production review requests.
    let trace = ExecutionTrace {
        steps: vec![ExecutionStep {
            step_id: "review-gate".to_string(),
            step_type: "workflow".to_string(),
            timestamp_ms: 0,
            input: Some(json!({ "task": task })),
            output: None,
            metadata: HashMap::new(),
        }],
        duration_ms: 180,
    };

    ReviewContext::new(trace, json!({ "request": task }))
}

async fn run_approved_flow() {
    let harness = HitlTestHarness::new();
    // Queue the reviewer decision before creating the review so the flow stays deterministic.
    harness.reviewer().push_decision(ScriptedDecision::Approve {
        comment: Some("approved for production".to_string()),
    });

    let review_id = harness
        .request_workflow_review(
            "hitl-exec-approve",
            "deploy_production",
            sample_context("Deploy to production"),
        )
        .await
        .expect("create workflow review");

    let decision = harness
        .resolve_with_script(&review_id)
        .await
        .expect("resolve workflow review");
    let response = harness
        .wait_for_review(&review_id, Duration::from_millis(50))
        .await
        .expect("wait for workflow review");
    let review = harness
        .get_review(&review_id)
        .await
        .expect("load workflow review")
        .expect("review exists");

    println!("== Approval Flow ==");
    println!("review_id: {}", review_id);
    println!("decision: {:?}", decision);
    println!("response: {:?}", response);
    println!("status: {:?}", review.status);
    println!("resolved_by: {:?}", review.resolved_by);
    println!("node_id: {:?}", review.node_id);
    println!();
}

async fn run_rejected_tool_flow() {
    let harness = HitlTestHarness::new();
    // Tool-call reviews attach tool metadata to the stored review request.
    harness.reviewer().push_decision(ScriptedDecision::Reject {
        reason: "manual verification required".to_string(),
        comment: Some("run staging validation first".to_string()),
    });

    let review_id = harness
        .request_tool_call_review(
            "hitl-exec-reject",
            "deployment_api",
            json!({ "environment": "prod", "version": "2026.03.17" }),
            sample_context("Call deployment_api for production rollout"),
        )
        .await
        .expect("create tool review");

    harness
        .resolve_with_script(&review_id)
        .await
        .expect("resolve tool review");
    let review = harness
        .get_review(&review_id)
        .await
        .expect("load tool review")
        .expect("review exists");

    println!("== Rejection Flow ==");
    println!("review_id: {}", review_id);
    println!("status: {:?}", review.status);
    println!("tool_name: {:?}", review.metadata.custom.get("tool_name"));
    println!("tool_args: {:?}", review.metadata.custom.get("tool_args"));
    println!("tags: {:?}", review.metadata.tags);
    println!();
}

async fn run_timeout_flow() {
    let harness = HitlTestHarness::new();
    // A timeout decision intentionally leaves the review unresolved so callers
    // can verify blocked execution paths.
    harness.reviewer().push_decision(ScriptedDecision::Timeout);

    let review_id = harness
        .request_workflow_review(
            "hitl-exec-timeout",
            "destructive_action",
            sample_context("Delete production index"),
        )
        .await
        .expect("create timeout review");

    let decision = harness
        .resolve_with_script(&review_id)
        .await
        .expect("apply timeout decision");
    let wait_result = harness
        .wait_for_review(&review_id, Duration::from_millis(50))
        .await;
    let review = harness
        .get_review(&review_id)
        .await
        .expect("load timeout review")
        .expect("review exists");

    println!("== Timeout Flow ==");
    println!("review_id: {}", review_id);
    println!("decision: {:?}", decision);
    println!("wait_result: {:?}", wait_result);
    println!("status: {:?}", review.status);
    println!("pending: {}", matches!(review.status, ReviewStatus::Pending));
    println!();
}

async fn run_deferred_flow() {
    let harness = HitlTestHarness::new();
    // A deferred decision records reviewer intent but keeps the review pending
    harness.reviewer().push_decision(ScriptedDecision::Defer {
        reason: "waiting for product sign-off".to_string(),
    });

    let review_id = harness
        .request_workflow_review(
            "hitl-exec-defer",
            "launch_gate",
            sample_context("Launch feature flag to 100%"),
        )
        .await
        .expect("create deferred review");

    let decision = harness
        .resolve_with_script(&review_id)
        .await
        .expect("apply deferred decision");
    let wait_result = harness
        .wait_for_review(&review_id, Duration::from_millis(50))
        .await;
    let pending_reviews = harness
        .list_pending_reviews(None)
        .await
        .expect("list pending reviews");
    let review = harness
        .get_review(&review_id)
        .await
        .expect("load deferred review")
        .expect("review exists");

    println!("== Deferred Flow ==");
    println!("review_id: {}", review_id);
    println!("decision: {:?}", decision);
    println!("wait_result: {:?}", wait_result);
    println!("status: {:?}", review.status);
    println!("pending_reviews: {}", pending_reviews.len());
    println!("deferred_response: {:?}", review.response);
    println!();
}

#[tokio::main]
async fn main() {
    run_approved_flow().await;
    run_rejected_tool_flow().await;
    run_timeout_flow().await;
    run_deferred_flow().await;
}
