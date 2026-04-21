//! Example: Gateway with custom proxy configuration
//!
//! This example demonstrates how to configure the gateway proxy with custom settings:
//! - Custom backend URL
//! - Custom timeouts
//! - Custom health check endpoints
//! - Environment variable configuration
//!
//! # Usage
//!
//! ```bash
//! # Set custom backend URL
//! export MOFA_LOCAL_LLM_URL="http://localhost:9000"
//!
//! # Run the example
//! cargo run --example proxy_with_custom_config
//! ```

use mofa_gateway::gateway::{Gateway, GatewayConfig};
use mofa_gateway::types::LoadBalancingAlgorithm;
use std::env;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("mofa_gateway=debug".parse().unwrap()),
        )
        .init();

    println!("🚀 Starting MoFA Gateway with Custom Proxy Configuration");
    println!("{}", "=".repeat(60));

    // Read configuration from environment or use defaults
    let backend_url =
        env::var("MOFA_LOCAL_LLM_URL").unwrap_or_else(|_| "http://localhost:8000".to_string());

    let listen_addr =
        env::var("GATEWAY_LISTEN_ADDR").unwrap_or_else(|_| "0.0.0.0:8080".to_string());

    println!("\n📋 Configuration:");
    println!("  Gateway Address: {}", listen_addr);
    println!("  Backend URL: {}", backend_url);
    println!("  Rate Limiting: Enabled");
    println!("  Circuit Breakers: Enabled");
    println!("  Health Checks: Enabled");

    // Create gateway configuration
    let config = GatewayConfig {
        listen_addr: listen_addr.parse()?,
        load_balancing: LoadBalancingAlgorithm::RoundRobin,
        enable_rate_limiting: true,
        enable_circuit_breakers: true,
        enable_local_llm_proxy: true,
        local_llm_backend_url: Some(backend_url.clone()),
    };

    // Create and start gateway
    let mut gateway = Gateway::new(config).await?;

    println!("\n✅ Gateway initialized successfully");
    println!("\n🔗 Available Endpoints:");
    println!("  Health:           http://{}/health", listen_addr);
    println!("  Metrics:          http://{}/metrics", listen_addr);
    println!("  Models List:      http://{}/v1/models", listen_addr);
    println!(
        "  Model Info:       http://{}/v1/models/{{model_id}}",
        listen_addr
    );
    println!(
        "  Chat Completions: http://{}/v1/chat/completions",
        listen_addr
    );

    println!("\n📝 Example Requests:");
    println!("  # List models");
    println!("  curl http://{}/v1/models", listen_addr);
    println!();
    println!("  # Get model info");
    println!(
        "  curl http://{}/v1/models/qwen2.5-0.5b-instruct",
        listen_addr
    );
    println!();
    println!("  # Chat completion");
    println!(
        "  curl -X POST http://{}/v1/chat/completions \\",
        listen_addr
    );
    println!("    -H 'Content-Type: application/json' \\");
    println!("    -d '{{");
    println!("      \"model\": \"qwen2.5-0.5b-instruct\",");
    println!("      \"messages\": [{{\"role\": \"user\", \"content\": \"Hello!\"}}],");
    println!("      \"max_tokens\": 100");
    println!("    }}'");

    println!("\n🔧 Advanced Configuration:");
    println!("  Set custom backend: export MOFA_LOCAL_LLM_URL=http://localhost:9000");
    println!("  Set custom port:    export GATEWAY_LISTEN_ADDR=0.0.0.0:8081");
    println!("  Enable debug logs:  export RUST_LOG=debug");

    println!("\n🎯 Starting gateway server...");
    gateway.start().await?;

    println!("\n✨ Gateway is running! Press Ctrl+C to stop.");

    // Keep running until interrupted
    tokio::signal::ctrl_c().await?;

    println!("\n\n🛑 Shutting down gateway...");
    gateway.stop().await?;

    println!("✅ Gateway stopped successfully");

    Ok(())
}
