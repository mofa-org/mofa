//! HTTP API Example for HITL system
//!
//! This example demonstrates how to use the HTTP API endpoints when http-api feature is enabled

#[cfg(feature = "http-api")]
use axum::Router;
#[cfg(feature = "http-api")]
use mofa_foundation::hitl::{
    ReviewManager, ReviewManagerConfig, ReviewNotifier, ReviewPolicyEngine,
    InMemoryReviewStore, ReviewApiState, create_review_api_router,
};
use mofa_kernel::hitl::{ReviewRequest, ReviewType, ReviewContext, ExecutionTrace};
use std::sync::Arc;
#[cfg(feature = "http-api")]
use tokio::net::TcpListener;

#[cfg(feature = "http-api")]
pub async fn test_http_api() -> anyhow::Result<()> {
    println!("=== HTTP API Example ===\n");
    println!("This example demonstrates the HTTP API endpoints for the HITL system.\n");

    // Setup review manager with audit store
    let store = Arc::new(InMemoryReviewStore::new());
    let audit_store = Arc::new(mofa_foundation::hitl::InMemoryAuditStore::new());
    let notifier = Arc::new(ReviewNotifier::default());
    let policy_engine = Arc::new(ReviewPolicyEngine::default());
    
    let manager = Arc::new(ReviewManager::with_audit_store(
        store,
        notifier,
        policy_engine,
        None,
        audit_store,
        ReviewManagerConfig::default(),
    ));

    // Create test reviews
    println!("[1] Creating test reviews...");
    let trace = ExecutionTrace {
        steps: vec![],
        duration_ms: 100,
    };
    let context = ReviewContext::new(trace, serde_json::json!({"test": "data"}));
    let review1 = ReviewRequest::new("test-exec-1", ReviewType::Approval, context.clone());
    let review_id1 = manager.request_review(review1).await?;
    println!("  ✓ Created review: {}", review_id1.as_str());

    let review2 = ReviewRequest::new("test-exec-2", ReviewType::Approval, context);
    let review_id2 = manager.request_review(review2).await?;
    println!("  ✓ Created review: {}\n", review_id2.as_str());

    // Setup API
    println!("[2] Setting up HTTP API server...");
    let api_state = Arc::new(ReviewApiState::new(manager.clone()));
    let app = Router::new()
        .nest("/api/reviews", create_review_api_router(api_state));
    println!("  ✓ API router created\n");

    // Start server in background
    println!("[3] Starting HTTP server on http://127.0.0.1:3000...");
    let listener = TcpListener::bind("127.0.0.1:3000").await?;
    println!("  ✓ Server listening on port 3000\n");

    // Start server in background task
    let app_for_server = app.clone();
    let server_handle = tokio::spawn(async move {
        axum::serve(listener, app_for_server).await
    });

    // Give server time to start
    tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;

    // Test endpoints using reqwest
    println!("[4] Testing API endpoints...");
    
    let client = reqwest::Client::new();
    
    // Test GET /api/reviews (list reviews)
    println!("  Testing GET /api/reviews...");
    let response = client
        .get("http://127.0.0.1:3000/api/reviews")
        .send()
        .await?;
    assert!(response.status().is_success(), "List reviews endpoint failed");
    let reviews: serde_json::Value = response.json().await?;
    assert_eq!(reviews["total"], 2, "Expected 2 reviews");
    println!("    ✓ List reviews: {} reviews found", reviews["total"]);

    // Test GET /api/reviews/:id (get specific review)
    println!("  Testing GET /api/reviews/{}...", review_id1.as_str());
    let response = client
        .get(&format!("http://127.0.0.1:3000/api/reviews/{}", review_id1.as_str()))
        .send()
        .await?;
    assert!(response.status().is_success(), "Get review endpoint failed");
    let review: serde_json::Value = response.json().await?;
    assert_eq!(review["id"], review_id1.as_str(), "Review ID mismatch");
    println!("    ✓ Get review: Found review {}", review["id"]);

    // Test POST /api/reviews/:id/resolve (resolve review)
    println!("  Testing POST /api/reviews/{}/resolve...", review_id1.as_str());
    let resolve_payload = serde_json::json!({
        "response": {
            "Approved": {
                "comment": "Looks good!"
            }
        },
        "resolved_by": "test@example.com"
    });
    let response = client
        .post(&format!("http://127.0.0.1:3000/api/reviews/{}/resolve", review_id1.as_str()))
        .json(&resolve_payload)
        .send()
        .await?;
    assert!(response.status().is_success(), "Resolve review endpoint failed");
    let result: serde_json::Value = response.json().await?;
    assert_eq!(result["message"], "Review resolved successfully");
    println!("    ✓ Resolve review: {}", result["message"]);

    // Test GET /api/reviews/:id/audit (get audit events)
    println!("  Testing GET /api/reviews/{}/audit...", review_id1.as_str());
    let response = client
        .get(&format!("http://127.0.0.1:3000/api/reviews/{}/audit", review_id1.as_str()))
        .send()
        .await?;
    assert!(response.status().is_success(), "Get audit events endpoint failed");
    let audit: serde_json::Value = response.json().await?;
    assert!(audit["total"].as_u64().unwrap() > 0, "Expected audit events");
    println!("    ✓ Get audit events: {} events found", audit["total"]);

    // Test GET /api/audit/events (query audit events)
    println!("  Testing GET /api/reviews/audit/events...");
    let response = client
        .get("http://127.0.0.1:3000/api/reviews/audit/events?limit=10")
        .send()
        .await?;
    assert!(response.status().is_success(), "Query audit events endpoint failed");
    let events: serde_json::Value = response.json().await?;
    assert!(events["total"].as_u64().unwrap() > 0, "Expected audit events");
    println!("    ✓ Query audit events: {} events found\n", events["total"]);

    println!("=== All HTTP API endpoints tested successfully! ===\n");
    
    // Stop server
    server_handle.abort();
    let _ = server_handle.await;
    println!("✓ HTTP API server stopped\n");
    
    Ok(())
}

#[cfg(not(feature = "http-api"))]
pub async fn test_http_api() -> anyhow::Result<()> {
    println!("HTTP API feature not enabled.");
    println!("To run HTTP API example, use: cargo run -p hitl_workflow --features http-api -- api-example");
    Ok(())
}
