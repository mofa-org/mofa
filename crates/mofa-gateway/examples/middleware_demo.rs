//! Middleware Chain Demo
//!
//! This example demonstrates how to use the middleware chain system in the gateway.
//! It shows how to configure logging, metrics, rate limiting, and cost tracking middleware.

use std::sync::Arc;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use axum::response::Response;
use futures::future::BoxFuture;

use mofa_gateway::middleware::{
    CostTracker, GatewayRateLimiter, LoggingMiddleware, MetricsMiddleware,
    Middleware, MiddlewareChain, Next, RequestContext, ResponseContext,
};

/// Custom handler that simulates an API call
async fn mock_api_handler(ctx: RequestContext) -> ResponseContext {
    // Simulate some processing
    let response = Response::builder()
        .status(StatusCode::OK)
        .header("Content-Type", "application/json")
        .header("x-mofa-model", "gpt-4o")
        .body(Body::from(r#"{"result": "success"}"#))
        .unwrap();

    ResponseContext::new(response)
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive(tracing::Level::INFO.into()),
        )
        .init();

    tracing::info!("Starting Middleware Chain Demo");

    // Create middleware chain with all middlewares
    let chain = MiddlewareChain::new()
        .add(LoggingMiddleware::new())
        .add(MetricsMiddleware::new())
        .add(GatewayRateLimiter::new(60)) // 60 requests per minute
        .add(CostTracker::new())
        .with_handler(|ctx| {
            Box::pin(async move { mock_api_handler(ctx).await })
        });

    // Test 1: Make a successful request
    tracing::info!("\n=== Test 1: Successful request ===");
    let request = Request::builder()
        .uri("/v1/chat/completions")
        .method("POST")
        .body(Body::from(
            r#"{"model": "gpt-4o", "messages": [{"role": "user", "content": "Hello!"}]}"#,
        ))
        .unwrap();

    let ctx = RequestContext::new(request);
    let response = chain.execute(ctx).await;

    tracing::info!("Response status: {:?}", response.response.status());
    tracing::info!(
        "x-mofa-cost-usd: {:?}",
        response
            .response
            .headers()
            .get("x-mofa-cost-usd")
            .map(|h| h.to_str().unwrap_or(""))
    );
    tracing::info!(
        "x-mofa-tokens-in: {:?}",
        response
            .response
            .headers()
            .get("x-mofa-tokens-in")
            .map(|h| h.to_str().unwrap_or(""))
    );
    tracing::info!(
        "x-mofa-tokens-out: {:?}",
        response
            .response
            .headers()
            .get("x-mofa-tokens-out")
            .map(|h| h.to_str().unwrap_or(""))
    );

    // Test 2: Test rate limiting (make multiple requests)
    tracing::info!("\n=== Test 2: Rate limiting ===");
    for i in 0..5 {
        let request = Request::builder()
            .uri("/v1/chat/completions")
            .method("POST")
            .body(Body::empty())
            .unwrap();

        let mut ctx = RequestContext::new(request);
        ctx.client_ip = Some(std::net::IpAddr::V4(std::net::Ipv4Addr::new(127, 0, 0, 1)));

        let response = chain.execute(ctx).await;
        tracing::info!(
            "Request {} - Status: {:?}",
            i + 1,
            response.response.status()
        );
    }

    tracing::info!("\n=== Demo Complete ===");
    tracing::info!("Middleware chain executed successfully!");
    tracing::info!("Check the console logs to see middleware in action.");
    tracing::info!("Cost headers should be present in responses.");

    Ok(())
}
