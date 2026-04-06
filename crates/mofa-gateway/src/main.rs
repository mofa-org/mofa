//! MoFA Gateway binary entrypoint

use mofa_gateway::openai_compat::{GatewayConfig, GatewayServer};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing for logging
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    // Configure the gateway
    let config = GatewayConfig::default()
        .with_port(8081)
        .with_api_key("sk-test-123")
        .with_rpm(60);

    // Create and start the server
    let server = GatewayServer::new(config);
    
    println!("Starting MoFA Gateway server...");
    println!("API will be available at http://127.0.0.1:8081");
    println!("POST /v1/chat/completions - Chat completions endpoint");
    println!("GET  /v1/models          - List available models");
    println!();
    println!("Example request includes cost headers in response:");
    println!("  x-mofa-cost-usd    - Cost in USD");
    println!("  x-mofa-tokens-in   - Input tokens");
    println!("  x-mofa-tokens-out  - Output tokens");
    
    // Start serving - this blocks until the server shuts down
    server.serve().await?;

    Ok(())
}
