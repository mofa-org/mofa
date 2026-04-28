//! Example: Gateway with mofa-local-llm proxy enabled.
//!
//! This example demonstrates how to start the gateway with mofa-local-llm
//! proxy functionality enabled. The gateway will forward requests to
//! mofa-local-llm's HTTP server.
//!
//! # Prerequisites
//!
//! - mofa-local-llm HTTP server should be running on http://localhost:8000
//!   (or configure via MOFA_LOCAL_LLM_URL environment variable)
//!
//! # Usage
//!
//! ```bash
//! # Start mofa-local-llm server first (in another terminal)
//! # cd mofa-local-llm && cargo run --bin server
//!
//! # Start gateway with local-llm proxy
//! cargo run --example gateway_local_llm_proxy
//!
//! # Test the proxy
//! curl http://localhost:8080/v1/models
//! curl -X POST http://localhost:8080/v1/chat/completions \
//!   -H "Content-Type: application/json" \
//!   -d '{"model": "qwen2.5-0.5b-instruct", "messages": [{"role": "user", "content": "Hello!"}]}'
//! ```

use mofa_gateway::gateway::{Gateway, GatewayConfig};
use std::time::Duration;
use tracing::{Level, info};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing
    tracing_subscriber::fmt().with_max_level(Level::INFO).init();

    info!("Starting MoFA Gateway with mofa-local-llm proxy");

    // Configure gateway
    let mut config = GatewayConfig::default();

    // Enable local-llm proxy (default is true, but explicit for clarity)
    config.enable_local_llm_proxy = true;

    // Optionally set custom backend URL (defaults to http://localhost:8000)
    // config.local_llm_backend_url = Some("http://localhost:8000".to_string());

    // Or use environment variable:
    // MOFA_LOCAL_LLM_URL=http://localhost:8000 cargo run --example gateway_local_llm_proxy
    // MOFA_LOCAL_LLM_ENABLED=true cargo run --example gateway_local_llm_proxy

    info!(
        listen_addr = %config.listen_addr,
        local_llm_enabled = config.enable_local_llm_proxy,
        local_llm_url = config.local_llm_backend_url.as_deref().unwrap_or("http://localhost:8000"),
        "Gateway configuration"
    );

    // Create and start gateway
    let mut gateway = Gateway::new(config.clone()).await?;
    gateway.start().await?;

    info!("Gateway started successfully!");
    info!("Gateway listening on http://{}", config.listen_addr);
    info!("mofa-local-llm proxy endpoints:");
    info!("  GET  /v1/models");
    info!("  GET  /v1/models/{{model_id}}");
    info!("  POST /v1/chat/completions");
    info!("");
    info!("Press Ctrl+C to stop");

    // Wait for shutdown signal
    tokio::signal::ctrl_c().await?;

    info!("Shutting down gateway...");
    gateway.stop().await?;

    info!("Gateway stopped");

    Ok(())
}
