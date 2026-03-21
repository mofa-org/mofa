//! OpenAI-Compatible Gateway Server Example
//!
//! This example demonstrates how to start a gateway HTTP server with OpenAI-compatible API:
//! - `POST /v1/chat/completions` - Chat completions (streaming and non-streaming)
//! - `GET /v1/models` - List available models
//!
//! # Usage
//!
//! ```bash
//! cargo run --example gateway_inference_server --features openai-compat
//! ```
//!
//! Then test streaming endpoint:
//! ```bash
//! curl -N http://localhost:8080/v1/chat/completions \
//!   -H "Content-Type: application/json" \
//!   -d '{"model":"gpt-4","stream":true,"messages":[{"role":"user","content":"Hello"}]}'
//! ```

use mofa_gateway::openai_compat::{GatewayConfig, GatewayServer};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive(tracing::Level::INFO.into()),
        )
        .init();

    tracing::info!("Starting MoFA OpenAI-Compatible Gateway Server");

    // Create gateway configuration
    let config = GatewayConfig::default()
        .with_port(8080)
        .with_rpm(120)
        .with_models(vec!["gpt-4o".to_string(), "qwen3-local".to_string()]);

    // Create and start the server
    let server = GatewayServer::new(config);
    server.serve().await?;

    Ok(())
}
