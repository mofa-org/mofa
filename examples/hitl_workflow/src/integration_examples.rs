//! Phase 2+ Integration Examples
//! 
//! Examples demonstrating the production-ready HITL system with ReviewManager,
//! workflow integration, tool execution integration, webhooks, and rate limiting.

use anyhow::Result;
use mofa_kernel::hitl::{
    ExecutionStep, ExecutionTrace, ReviewContext, ReviewRequest, ReviewType, ReviewResponse,
};
use mofa_foundation::hitl::{
    ReviewManager, ReviewManagerConfig, ReviewNotifier,
    ReviewPolicyEngine, RateLimiter, InMemoryReviewStore,
    WorkflowReviewHandler, ToolReviewHandler, WebhookDelivery, WebhookConfig,
};
use std::sync::Arc;
use tracing::info;
use uuid::Uuid;

/// Run integration examples (Phase 2+: Foundation layer)
pub async fn run_integration_examples(example: &str) -> Result<()> {
    info!("=== HITL Integration Examples (Phase 2+) ===\n");
    info!("These examples demonstrate the production-ready HITL system:");
    info!("  - ReviewManager orchestration");
    info!("  - Workflow integration");
    info!("  - Tool execution integration");
    info!("  - Webhook notifications");
    info!("  - Rate limiting");
    info!("  - Multi-tenant isolation");
    info!("  - End-to-end workflow\n");

    match example {
        "manager" => example_review_manager().await?,
        "workflow" => example_workflow_integration().await?,
        "tool" => example_tool_integration().await?,
        "webhook" => example_webhook_notifications().await?,
        "rate_limit" => example_rate_limiting().await?,
        "multi_tenant" => example_multi_tenant().await?,
        "end_to_end" => example_end_to_end().await?,
        "all" => {
            example_review_manager().await?;
            example_workflow_integration().await?;
            example_tool_integration().await?;
            example_webhook_notifications().await?;
            example_rate_limiting().await?;
            example_multi_tenant().await?;
            example_end_to_end().await?;
        }
        _ => {
            return Err(anyhow::anyhow!(
                "Unknown example: {}. Use: manager, workflow, tool, webhook, rate_limit, multi_tenant, end_to_end, or all",
                example
            ));
        }
    }

    info!("\n=== Integration examples completed ===");
    Ok(())
}

/// Example: ReviewManager Usage
async fn example_review_manager() -> Result<()> {
    info!("\n--- Integration Example 1: ReviewManager ---");

    // Create components
    let store = Arc::new(InMemoryReviewStore::new());
    let notifier = Arc::new(ReviewNotifier::default());
    let policy_engine = Arc::new(ReviewPolicyEngine::default());
    let rate_limiter = Some(Arc::new(RateLimiter::default()));

    // Create ReviewManager
    let manager = Arc::new(ReviewManager::new(
        store,
        notifier,
        policy_engine,
        rate_limiter,
        ReviewManagerConfig::default(),
    ));

    // Create a review request
    let review = ReviewRequest::new(
        "exec_manager_demo",
        ReviewType::Approval,
        create_sample_context(),
    )
    .with_node_id("approval_node");

    info!("Creating review request: {}", review.id.as_str());
    let review_id = manager.request_review(review).await
        .map_err(|e| anyhow::anyhow!("Failed to create review: {}", e))?;
    info!("Review created with ID: {}", review_id.as_str());

    // Get the review
    if let Some(review) = manager.get_review(&review_id).await
        .map_err(|e| anyhow::anyhow!("Failed to get review: {}", e))? {
        info!("Retrieved review: status = {:?}", review.status);
    }

    // List pending reviews
    let pending = manager.list_pending(None, Some(10)).await
        .map_err(|e| anyhow::anyhow!("Failed to list pending reviews: {}", e))?;
    info!("Pending reviews: {}", pending.len());

    // Resolve the review
    info!("Resolving review...");
    manager.resolve_review(
        &review_id,
        ReviewResponse::Approved {
            comment: Some("Looks good!".to_string()),
        },
        "reviewer@example.com".to_string(),
    ).await
        .map_err(|e| anyhow::anyhow!("Failed to resolve review: {}", e))?;

    info!("Review resolved successfully!");
    Ok(())
}

/// Example: Workflow Integration
async fn example_workflow_integration() -> Result<()> {
    info!("\n--- Integration Example 2: Workflow Integration ---");

    // Create ReviewManager
    let store = Arc::new(InMemoryReviewStore::new());
    let notifier = Arc::new(ReviewNotifier::default());
    let policy_engine = Arc::new(ReviewPolicyEngine::default());
    let manager = Arc::new(ReviewManager::new(
        store,
        notifier,
        policy_engine,
        None, // No rate limiting for this example
        ReviewManagerConfig::default(),
    ));

    // Create workflow handler
    let handler = WorkflowReviewHandler::new(Arc::clone(&manager));

    // Simulate workflow execution requesting review at a node
    info!("Workflow execution 'workflow_123' reaching review node 'payment_approval'");
    let review_id = handler.request_node_review(
        "workflow_123",
        "payment_approval",
        create_sample_context(),
    ).await
        .map_err(|e| anyhow::anyhow!("Failed to request workflow review: {}", e))?;

    info!("Review requested: {}", review_id.as_str());
    info!("Workflow paused, waiting for review...");

    // Simulate review resolution
    manager.resolve_review(
        &review_id,
        ReviewResponse::Approved {
            comment: Some("Payment approved".to_string()),
        },
        "finance_team@example.com".to_string(),
    ).await
        .map_err(|e| anyhow::anyhow!("Failed to resolve review: {}", e))?;

    // Check if resolved
    if handler.is_resolved(&review_id).await
        .map_err(|e| anyhow::anyhow!("Failed to check review status: {}", e))? {
        info!("Review resolved! Workflow can continue.");
    }

    Ok(())
}

/// Example: Tool Execution Integration
async fn example_tool_integration() -> Result<()> {
    info!("\n--- Integration Example 3: Tool Execution Integration ---");

    // Create ReviewManager
    let store = Arc::new(InMemoryReviewStore::new());
    let notifier = Arc::new(ReviewNotifier::default());
    let policy_engine = Arc::new(ReviewPolicyEngine::default());
    let manager = Arc::new(ReviewManager::new(
        store,
        notifier,
        policy_engine,
        None,
        ReviewManagerConfig::default(),
    ));

    // Create tool handler
    let handler = ToolReviewHandler::new(Arc::clone(&manager));

    // Simulate tool call requiring review
    info!("Tool 'database_delete' called with sensitive operation");
    let review_id = handler.request_tool_call_review(
        "exec_tool_demo",
        "database_delete",
        serde_json::json!({
            "table": "users",
            "condition": "id = 12345"
        }),
        create_sample_context(),
    ).await
        .map_err(|e| anyhow::anyhow!("Failed to request tool review: {}", e))?;

    info!("Tool call review requested: {}", review_id.as_str());
    info!("Tool execution paused, waiting for approval...");

    // Simulate approval
    manager.resolve_review(
        &review_id,
        ReviewResponse::Approved {
            comment: Some("Delete operation approved".to_string()),
        },
        "dba@example.com".to_string(),
    ).await
        .map_err(|e| anyhow::anyhow!("Failed to resolve review: {}", e))?;

    info!("Tool call approved! Execution can proceed.");

    // Example: Review tool output
    info!("\nReviewing tool output...");
    let output_review_id = handler.request_tool_output_review(
        "exec_tool_demo",
        "database_delete",
        serde_json::json!({
            "rows_deleted": 1,
            "status": "success"
        }),
        create_sample_context(),
    ).await
        .map_err(|e| anyhow::anyhow!("Failed to request tool output review: {}", e))?;

    info!("Tool output review requested: {}", output_review_id.as_str());
    manager.resolve_review(
        &output_review_id,
        ReviewResponse::Approved {
            comment: None,
        },
        "dba@example.com".to_string(),
    ).await
        .map_err(|e| anyhow::anyhow!("Failed to resolve review: {}", e))?;

    info!("Tool output approved!");
    Ok(())
}

/// Example: Webhook Notifications
async fn example_webhook_notifications() -> Result<()> {
    info!("\n--- Integration Example 4: Webhook Notifications ---");

    // Create webhook config (using a mock URL for demo)
    let webhook_config = WebhookConfig {
        url: "https://example.com/webhook".to_string(),
        secret: Some("webhook_secret_key".to_string()),
        max_retries: 3,
        retry_delay: std::time::Duration::from_secs(1),
        timeout: std::time::Duration::from_secs(10),
    };

    let webhook = WebhookDelivery::new(webhook_config);

    // Create a review request
    let review = ReviewRequest::new(
        "exec_webhook_demo",
        ReviewType::Approval,
        create_sample_context(),
    );

    info!("Sending webhook notification for review: {}", review.id.as_str());
    
    // Note: This will fail in the example since the URL doesn't exist,
    // but it demonstrates the API
    match webhook.deliver(&review, "review.created").await {
        Ok(_) => info!("Webhook delivered successfully!"),
        Err(e) => {
            info!("Webhook delivery (expected to fail with mock URL): {}", e);
            info!("In production, configure a real webhook URL.");
        }
    }

    Ok(())
}

/// Example: Rate Limiting
async fn example_rate_limiting() -> Result<()> {
    info!("\n--- Integration Example 5: Rate Limiting ---");

    // Create rate limiter (10 requests/sec, max 100 tokens)
    let rate_limiter = RateLimiter::new(10.0, 100.0);

    info!("Testing rate limiter (10 req/sec, max 100 tokens)");
    info!("Tenant: 'tenant_1'");

    // Make several requests
    let mut success_count = 0;
    let mut rate_limited_count = 0;

    for i in 1..=15 {
        match rate_limiter.check("tenant_1").await {
            Ok(_) => {
                success_count += 1;
                if i <= 10 {
                    info!("Request {}: Allowed", i);
                }
            }
            Err(e) => {
                rate_limited_count += 1;
                if i == 11 {
                    info!("Request {}: Rate limited: {}", i, e);
                }
            }
        }
        // Small delay to simulate real requests
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }

    info!("\nRate limiting results:");
    info!("  Successful requests: {}", success_count);
    info!("  Rate limited requests: {}", rate_limited_count);
    info!("  (First 10 should succeed, then rate limiting kicks in)");

    Ok(())
}

/// Example: Multi-Tenant Isolation
async fn example_multi_tenant() -> Result<()> {
    info!("\n--- Integration Example 6: Multi-Tenant Isolation ---");

    // Create ReviewManager
    let store = Arc::new(InMemoryReviewStore::new());
    let notifier = Arc::new(ReviewNotifier::default());
    let policy_engine = Arc::new(ReviewPolicyEngine::default());
    let manager = Arc::new(ReviewManager::new(
        store,
        notifier,
        policy_engine,
        None,
        ReviewManagerConfig::default(),
    ));

    // Create reviews for different tenants
    let tenant_1 = Uuid::new_v4();
    let tenant_2 = Uuid::new_v4();

    info!("Creating reviews for two tenants:");
    info!("  Tenant 1: {}", tenant_1);
    info!("  Tenant 2: {}", tenant_2);

    // Create review for tenant 1
    let mut review_1 = ReviewRequest::new(
        "exec_tenant_1",
        ReviewType::Approval,
        create_sample_context(),
    );
    review_1.metadata.tenant_id = Some(tenant_1);
    let review_id_1 = manager.request_review(review_1).await
        .map_err(|e| anyhow::anyhow!("Failed to create review: {}", e))?;
    info!("  Created review {} for tenant 1", review_id_1.as_str());

    // Create review for tenant 2
    let mut review_2 = ReviewRequest::new(
        "exec_tenant_2",
        ReviewType::Approval,
        create_sample_context(),
    );
    review_2.metadata.tenant_id = Some(tenant_2);
    let review_id_2 = manager.request_review(review_2).await
        .map_err(|e| anyhow::anyhow!("Failed to create review: {}", e))?;
    info!("  Created review {} for tenant 2", review_id_2.as_str());

    // List pending reviews for tenant 1 (should only see tenant 1's review)
    let tenant_1_reviews = manager.list_pending(Some(tenant_1), Some(10)).await
        .map_err(|e| anyhow::anyhow!("Failed to list reviews: {}", e))?;
    info!("\nTenant 1's pending reviews: {}", tenant_1_reviews.len());
    for review in &tenant_1_reviews {
        info!("  - {} (execution: {})", review.id.as_str(), review.execution_id);
    }

    // List pending reviews for tenant 2 (should only see tenant 2's review)
    let tenant_2_reviews = manager.list_pending(Some(tenant_2), Some(10)).await
        .map_err(|e| anyhow::anyhow!("Failed to list reviews: {}", e))?;
    info!("\nTenant 2's pending reviews: {}", tenant_2_reviews.len());
    for review in &tenant_2_reviews {
        info!("  - {} (execution: {})", review.id.as_str(), review.execution_id);
    }

    // Verify isolation: tenant 1 cannot see tenant 2's review
    assert_eq!(tenant_1_reviews.len(), 1, "Tenant 1 should see 1 review");
    assert_eq!(tenant_2_reviews.len(), 1, "Tenant 2 should see 1 review");
    assert_ne!(
        tenant_1_reviews[0].id, tenant_2_reviews[0].id,
        "Tenants should see different reviews"
    );

    info!("\nMulti-tenant isolation verified!");
    info!("   Each tenant can only see their own reviews.");

    Ok(())
}

/// Example: End-to-End Workflow
async fn example_end_to_end() -> Result<()> {
    info!("\n--- Integration Example 7: End-to-End Workflow ---");
    info!("This example demonstrates a complete workflow:");
    info!("  1. Workflow execution starts");
    info!("  2. Reaches a review node");
    info!("  3. Review is requested and workflow pauses");
    info!("  4. Human reviewer approves");
    info!("  5. Workflow resumes and completes");

    // Create ReviewManager with rate limiting
    let store = Arc::new(InMemoryReviewStore::new());
    let notifier = Arc::new(ReviewNotifier::default());
    let policy_engine = Arc::new(ReviewPolicyEngine::default());
    let rate_limiter = Some(Arc::new(RateLimiter::new(10.0, 100.0)));
    let manager = Arc::new(ReviewManager::new(
        store,
        notifier,
        policy_engine,
        rate_limiter,
        ReviewManagerConfig::default(),
    ));

    // Create workflow handler
    let handler = WorkflowReviewHandler::new(Arc::clone(&manager));

    // Step 1: Workflow execution starts
    info!("\n[Step 1] Workflow execution 'payment_workflow_001' starts");
    info!("  Processing payment of $5000.00");

    // Step 2: Workflow reaches review node
    info!("\n[Step 2] Workflow reaches review node 'approve_payment'");
    info!("  Payment exceeds threshold ($1000), review required");

    // Step 3: Request review
    let review_id = handler.request_node_review(
        "payment_workflow_001",
        "approve_payment",
        create_sample_context(),
    ).await
        .map_err(|e| anyhow::anyhow!("Failed to request review: {}", e))?;

    info!("\n[Step 3] Review requested: {}", review_id.as_str());
    info!("  Workflow paused, waiting for human approval...");

    // Simulate review details
    if let Some(review) = manager.get_review(&review_id).await
        .map_err(|e| anyhow::anyhow!("Failed to get review: {}", e))? {
        info!("  Review details:");
        info!("    Execution ID: {}", review.execution_id);
        info!("    Node ID: {:?}", review.node_id);
        info!("    Type: {:?}", review.review_type);
        info!("    Status: {:?}", review.status);
    }

    // Step 4: Human reviewer approves
    info!("\n[Step 4] Human reviewer 'finance@example.com' reviews request");
    info!("  Reviewer checks payment details and approves");

    manager.resolve_review(
        &review_id,
        ReviewResponse::Approved {
            comment: Some("Payment approved after verification".to_string()),
        },
        "finance@example.com".to_string(),
    ).await
        .map_err(|e| anyhow::anyhow!("Failed to resolve review: {}", e))?;

        info!("  Review approved!");

    // Step 5: Workflow resumes
    info!("\n[Step 5] Workflow resumes after approval");
    if handler.is_resolved(&review_id).await
        .map_err(|e| anyhow::anyhow!("Failed to check review status: {}", e))? {
        info!("  Review is resolved, workflow can continue");
        info!("  Processing payment...");
        info!("  Payment processed successfully!");
    }

    info!("\nEnd-to-end workflow completed successfully!");
    info!("   All steps executed: request → pause → review → approve → resume");

    Ok(())
}

/// Helper function to create a sample review context
fn create_sample_context() -> ReviewContext {
    use std::collections::HashMap;
    
    let trace = ExecutionTrace {
        steps: vec![ExecutionStep {
            step_id: "sample_step".to_string(),
            step_type: "sample".to_string(),
            timestamp_ms: 0,
            input: Some(serde_json::json!({"test": "data"})),
            output: Some(serde_json::json!({"result": "success"})),
            metadata: HashMap::new(),
        }],
        duration_ms: 100,
    };

    ReviewContext::new(
        trace,
        serde_json::json!({"example": "input"}),
    )
}
