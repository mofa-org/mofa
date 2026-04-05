//! HITL Integration Tests
//!
//! Integration tests for the Human-in-the-Loop system

use mofa_foundation::hitl::{
    InMemoryReviewStore, RateLimiter, ReviewManager, ReviewManagerConfig, ReviewNotifier,
    ReviewPolicyEngine, ToolReviewHandler, WorkflowReviewHandler,
};
use mofa_kernel::hitl::{
    ExecutionStep, ExecutionTrace, ReviewContext, ReviewRequest, ReviewResponse, ReviewStatus,
    ReviewType,
};
use std::sync::Arc;
use tokio::time::{Duration, sleep};

/// Helper to create a test review manager
fn create_test_manager() -> Arc<ReviewManager> {
    let store = Arc::new(InMemoryReviewStore::new());
    let notifier = Arc::new(ReviewNotifier::default());
    let policy_engine = Arc::new(ReviewPolicyEngine::default());

    Arc::new(ReviewManager::new(
        store,
        notifier,
        policy_engine,
        None, // No rate limiting for tests
        ReviewManagerConfig::default(),
    ))
}

/// Helper to create a test review context
fn create_test_context() -> ReviewContext {
    use std::collections::HashMap;

    let trace = ExecutionTrace {
        steps: vec![ExecutionStep {
            step_id: "test_step".to_string(),
            step_type: "test".to_string(),
            timestamp_ms: 0,
            input: None,
            output: None,
            metadata: HashMap::new(),
        }],
        duration_ms: 100,
    };

    ReviewContext::new(trace, serde_json::json!({"test": "data"}))
}

#[tokio::test]
async fn test_review_manager_basic_flow() {
    let manager = create_test_manager();

    // Create review
    let review = ReviewRequest::new("exec-1", ReviewType::Approval, create_test_context());
    let review_id = manager.request_review(review).await.expect("failed");

    // Verify pending
    let pending = manager.list_pending(None, None).await.expect("failed");
    assert_eq!(pending.len(), 1);
    assert_eq!(pending[0].id, review_id);

    // Resolve review
    manager
        .resolve_review(
            &review_id,
            ReviewResponse::Approved { comment: None },
            "reviewer@test.com".to_string(),
        )
        .await
        .unwrap();

    // Verify resolved
    let review = manager.get_review(&review_id).await.expect("failed").unwrap();
    assert_eq!(review.status, ReviewStatus::Approved);
}

#[tokio::test]
async fn test_workflow_review_handler() {
    let manager = create_test_manager();
    let handler = WorkflowReviewHandler::new(manager.clone());

    // Request review
    let review_id = handler
        .request_node_review("exec-1", "node-1", create_test_context())
        .await
        .unwrap();

    // Check not resolved
    assert!(!handler.is_resolved(&review_id).await.expect("failed"));

    // Resolve
    manager
        .resolve_review(
            &review_id,
            ReviewResponse::Approved { comment: None },
            "reviewer@test.com".to_string(),
        )
        .await
        .unwrap();

    // Check resolved
    assert!(handler.is_resolved(&review_id).await.expect("failed"));
    assert!(handler.is_approved(&review_id).await.expect("failed"));
}

#[tokio::test]
async fn test_tool_review_handler() {
    let manager = create_test_manager();
    let handler = ToolReviewHandler::new(manager.clone());

    // Request tool call review
    let review_id = handler
        .request_tool_call_review(
            "exec-1",
            "database_delete",
            serde_json::json!({"table": "users"}),
            create_test_context(),
        )
        .await
        .unwrap();

    // Verify review created
    let review = manager.get_review(&review_id).await.expect("failed").unwrap();
    assert_eq!(review.execution_id, "exec-1");
    assert!(review.metadata.tags.contains(&"tool_execution".to_string()));

    // Approve
    manager
        .resolve_review(
            &review_id,
            ReviewResponse::Approved {
                comment: Some("OK".to_string()),
            },
            "dba@test.com".to_string(),
        )
        .await
        .unwrap();

    // Verify approved
    assert!(handler.wait_for_review(&review_id).await.is_ok());
}

#[tokio::test]
async fn test_multi_tenant_isolation() {
    use uuid::Uuid;

    let manager = create_test_manager();
    let tenant_1 = Uuid::new_v4();
    let tenant_2 = Uuid::new_v4();

    // Create reviews for different tenants
    let mut review_1 = ReviewRequest::new("exec-1", ReviewType::Approval, create_test_context());
    review_1.metadata.tenant_id = Some(tenant_1);
    let id_1 = manager.request_review(review_1).await.expect("failed");

    let mut review_2 = ReviewRequest::new("exec-2", ReviewType::Approval, create_test_context());
    review_2.metadata.tenant_id = Some(tenant_2);
    let id_2 = manager.request_review(review_2).await.expect("failed");

    // List for tenant_1
    let tenant_1_reviews = manager.list_pending(Some(tenant_1), None).await.expect("failed");
    assert_eq!(tenant_1_reviews.len(), 1);
    assert_eq!(tenant_1_reviews[0].id, id_1);

    // List for tenant_2
    let tenant_2_reviews = manager.list_pending(Some(tenant_2), None).await.expect("failed");
    assert_eq!(tenant_2_reviews.len(), 1);
    assert_eq!(tenant_2_reviews[0].id, id_2);

    // Verify isolation
    assert_ne!(id_1, id_2);
}

#[tokio::test]
async fn test_rate_limiter() {
    let limiter = RateLimiter::new(2.0, 2.0); // 2 req/sec, max 2 tokens

    // First 2 should succeed
    assert!(limiter.check("tenant-1").await.is_ok());
    assert!(limiter.check("tenant-1").await.is_ok());

    // Third should be rate limited
    assert!(limiter.check("tenant-1").await.is_err());

    // Wait for refill
    sleep(Duration::from_millis(1100)).await;

    // Should succeed again
    assert!(limiter.check("tenant-1").await.is_ok());
}

#[tokio::test]
async fn test_review_lifecycle() {
    let manager = create_test_manager();

    // Create review
    let review = ReviewRequest::new(
        "exec-lifecycle",
        ReviewType::Approval,
        create_test_context(),
    );
    let review_id = manager.request_review(review).await.expect("failed");

    // Verify pending
    let review = manager.get_review(&review_id).await.expect("failed").unwrap();
    assert_eq!(review.status, ReviewStatus::Pending);

    // Reject review
    manager
        .resolve_review(
            &review_id,
            ReviewResponse::Rejected {
                reason: "Not approved".to_string(),
                comment: Some("Needs more info".to_string()),
            },
            "reviewer@test.com".to_string(),
        )
        .await
        .unwrap();

    // Verify rejected
    let review = manager.get_review(&review_id).await.expect("failed").unwrap();
    assert_eq!(review.status, ReviewStatus::Rejected);
    assert!(review.resolved_by.is_some());
}
