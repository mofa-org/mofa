use mofa_kernel::hitl::{ExecutionStep, ExecutionTrace, ReviewContext, ReviewResponse, ReviewStatus};
use mofa_testing::{HitlTestHarness, ScriptedDecision};
use serde_json::json;
use std::collections::HashMap;
use std::time::Duration;

fn sample_context() -> ReviewContext {
    // Keep the context small in tests, but include a real execution trace so we
    // validate the harness against actual HITL request shapes.
    let trace = ExecutionTrace {
        steps: vec![ExecutionStep {
            step_id: "deploy".to_string(),
            step_type: "workflow".to_string(),
            timestamp_ms: 0,
            input: Some(json!({"task": "deploy"})),
            output: None,
            metadata: HashMap::new(),
        }],
        duration_ms: 120,
    };

    ReviewContext::new(trace, json!({"request": "deploy to production"}))
}

#[tokio::test]
async fn workflow_review_can_be_approved_with_scripted_decision() {
    let harness = HitlTestHarness::new();
    // Script the next reviewer action up front so the test does not depend on timing.
    harness.reviewer().push_decision(ScriptedDecision::Approve {
        comment: Some("approved for rollout".to_string()),
    });

    let review_id = harness
        .request_workflow_review("exec-1", "deploy_node", sample_context())
        .await
        .unwrap();

    let applied = harness.resolve_with_script(&review_id).await.unwrap();
    assert_eq!(
        applied,
        ScriptedDecision::Approve {
            comment: Some("approved for rollout".to_string()),
        }
    );

    let response = harness
        .wait_for_review(&review_id, Duration::from_millis(50))
        .await
        .unwrap();
    assert!(matches!(
        response,
        ReviewResponse::Approved { comment }
            if comment.as_deref() == Some("approved for rollout")
    ));

    let review = harness.get_review(&review_id).await.unwrap().unwrap();
    assert_eq!(review.status, ReviewStatus::Approved);
    assert_eq!(review.node_id.as_deref(), Some("deploy_node"));
    assert_eq!(review.resolved_by.as_deref(), Some("hitl-test-reviewer"));
    assert!(harness.is_approved(&review_id).await.unwrap());
}

#[tokio::test]
async fn tool_call_review_can_be_rejected_and_preserve_metadata() {
    let harness = HitlTestHarness::new();
    // Rejections are useful because they verify both resolution status and
    // preservation of tool-specific metadata on the stored review.
    harness.reviewer().push_decision(ScriptedDecision::Reject {
        reason: "production deployment requires manual sign-off".to_string(),
        comment: Some("run staging first".to_string()),
    });

    let review_id = harness
        .request_tool_call_review(
            "exec-2",
            "deployment_api",
            json!({"environment": "prod"}),
            sample_context(),
        )
        .await
        .unwrap();

    harness.resolve_with_script(&review_id).await.unwrap();

    let review = harness.get_review(&review_id).await.unwrap().unwrap();
    assert_eq!(review.status, ReviewStatus::Rejected);
    assert!(review.metadata.tags.iter().any(|tag| tag == "tool_execution"));
    assert_eq!(
        review.metadata.custom.get("tool_name"),
        Some(&json!("deployment_api"))
    );
    assert_eq!(
        review.metadata.custom.get("tool_args"),
        Some(&json!({"environment": "prod"}))
    );
}

#[tokio::test]
async fn scripted_decisions_are_applied_in_order() {
    let harness = HitlTestHarness::new();
    harness.reviewer().push_decision(ScriptedDecision::Reject {
        reason: "first attempt denied".to_string(),
        comment: None,
    });
    harness.reviewer().push_decision(ScriptedDecision::Approve {
        comment: Some("second attempt accepted".to_string()),
    });

    let first = harness
        .request_workflow_review("exec-seq", "node-a", sample_context())
        .await
        .unwrap();
    let second = harness
        .request_workflow_review("exec-seq", "node-b", sample_context())
        .await
        .unwrap();

    let first_decision = harness.resolve_with_script(&first).await.unwrap();
    let second_decision = harness.resolve_with_script(&second).await.unwrap();

    assert!(matches!(first_decision, ScriptedDecision::Reject { .. }));
    assert!(matches!(second_decision, ScriptedDecision::Approve { .. }));
    assert_eq!(
        harness.review_status(&first).await.unwrap(),
        Some(ReviewStatus::Rejected)
    );
    assert_eq!(
        harness.review_status(&second).await.unwrap(),
        Some(ReviewStatus::Approved)
    );
    assert_eq!(harness.reviewer().pending_decisions(), 0);
}

#[tokio::test]
async fn timeout_decision_leaves_review_pending() {
    let harness = HitlTestHarness::new();
    // Timeout is modeled as "no reviewer response", so the review should remain pending.
    harness.reviewer().push_decision(ScriptedDecision::Timeout);

    let review_id = harness
        .request_workflow_review("exec-timeout", "node-timeout", sample_context())
        .await
        .unwrap();

    let applied = harness.resolve_with_script(&review_id).await.unwrap();
    assert_eq!(applied, ScriptedDecision::Timeout);

    let err = harness
        .wait_for_review(&review_id, Duration::from_millis(50))
        .await
        .unwrap_err();
    let message = err.to_string();
    assert!(message.contains("timed out"));

    let review = harness.get_review(&review_id).await.unwrap().unwrap();
    assert_eq!(review.status, ReviewStatus::Pending);
    assert!(review.response.is_none());
}

#[tokio::test]
async fn deferred_decision_stays_pending_and_does_not_finish_wait() {
    let harness = HitlTestHarness::new();
    harness.reviewer().push_decision(ScriptedDecision::Defer {
        reason: "needs product review".to_string(),
    });

    let review_id = harness
        .request_workflow_review("exec-defer", "node-defer", sample_context())
        .await
        .unwrap();

    let applied = harness.resolve_with_script(&review_id).await.unwrap();
    assert!(matches!(applied, ScriptedDecision::Defer { .. }));

    let err = harness
        .wait_for_review(&review_id, Duration::from_millis(50))
        .await
        .unwrap_err();
    assert!(err.to_string().contains("timed out"));

    let review = harness.get_review(&review_id).await.unwrap().unwrap();
    assert_eq!(review.status, ReviewStatus::Pending);
    assert!(matches!(review.response, Some(ReviewResponse::Deferred { .. })));
}

#[tokio::test]
async fn reviewer_default_decision_is_configurable_through_harness() {
    let harness = HitlTestHarness::new();
    harness
        .reviewer()
        .set_default_decision(ScriptedDecision::Reject {
            reason: "default deny".to_string(),
            comment: None,
        });

    let review_id = harness
        .request_workflow_review("exec-default", "node-default", sample_context())
        .await
        .unwrap();

    let applied = harness.resolve_with_script(&review_id).await.unwrap();
    assert!(matches!(applied, ScriptedDecision::Reject { .. }));
    assert_eq!(
        harness.review_status(&review_id).await.unwrap(),
        Some(ReviewStatus::Rejected)
    );
}

#[tokio::test]
async fn tool_output_review_preserves_output_metadata() {
    let harness = HitlTestHarness::new();
    harness.reviewer().push_decision(ScriptedDecision::Approve {
        comment: Some("output accepted".to_string()),
    });

    let review_id = harness
        .request_tool_output_review(
            "exec-tool-output",
            "deployment_api",
            json!({"result": "ok", "release_id": "rel-42"}),
            sample_context(),
        )
        .await
        .unwrap();

    harness.resolve_with_script(&review_id).await.unwrap();

    let review = harness.get_review(&review_id).await.unwrap().unwrap();
    assert_eq!(review.status, ReviewStatus::Approved);
    assert!(review.metadata.tags.iter().any(|tag| tag == "tool_output"));
    assert_eq!(
        review.metadata.custom.get("tool_name"),
        Some(&json!("deployment_api"))
    );
    assert_eq!(
        review.metadata.custom.get("tool_output"),
        Some(&json!({"result": "ok", "release_id": "rel-42"}))
    );
}
