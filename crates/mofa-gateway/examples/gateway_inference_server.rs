//! OpenAI-Compatible Inference Gateway Server Example
//!
//! This example demonstrates how to start the MoFA inference gateway server
//! which provides OpenAI-compatible endpoints:
//! - `GET /v1/models` - List available models
//! - `POST /v1/chat/completions` - Chat completions (non-streaming and streaming)
//!
//! # Usage
//!
//! ```bash
//! cargo run -p mofa-gateway --features openai-compat --example gateway_inference_server
//! ```
//!
//! Then test the endpoints:
//! ```bash
//! curl http://127.0.0.1:8080/v1/models
//! curl -X POST http://127.0.0.1:8080/v1/chat/completions \
//!   -H "Content-Type: application/json" \
//!   -d '{"model": "gpt-4", "messages": [{"role": "user", "content": "Hello!"}]}'
//! ```

use mofa_gateway::openai_compat::{GatewayConfig, GatewayServer};
use tokio::signal;
use tracing::{info, warn};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive(tracing::Level::INFO.into()),
        )
        .init();

    info!("Starting MoFA OpenAI-Compatible Inference Gateway");

    // Create gateway configuration with multiple models
    let config = GatewayConfig::default()
        .with_port(8080)
        .with_rpm(120)
        .with_models(vec![
            "gpt-4".to_string(),
            "claude-3".to_string(),
            "local-llama".to_string(),
        ]);

    info!("Gateway configuration:");
    info!("  Port: {}", config.port);
    info!("  Rate limit: {} requests/minute", config.rate_limit_rpm);
    info!("  Available models: {:?}", config.available_models);

    // Create and start the gateway server
    let server = GatewayServer::new(config);

    info!("Available endpoints:");
    info!("  GET  /v1/models           - List available models");
    info!("  POST /v1/chat/completions - Chat completions (non-streaming)");
    info!("  POST /v1/chat/completions - Chat completions (streaming, with stream: true)");

    // Wait for shutdown signal
    let ctrl_c = async {
        signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("failed to install signal handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {
            info!("Received Ctrl+C, shutting down gracefully...");
        },
        _ = terminate => {
            info!("Received terminate signal, shutting down gracefully...");
        },
        result = server.serve() => {
            if let Err(e) = result {
                warn!("Server error: {}", e);
            }
        }
    }

    info!("Gateway server stopped");

    Ok(())
}
