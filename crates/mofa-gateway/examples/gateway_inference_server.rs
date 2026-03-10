//! Gateway Server with Inference Bridge Example
//!
//! This example demonstrates a gateway server with the OpenAI-compatible
//! inference bridge endpoint.
//!
//! # Usage
//!
//! ```bash
//! cargo run -p mofa-gateway --example gateway_inference_server
//! ```
//!
//! Then test the endpoints:
//! ```powershell
//! # PowerShell
//! Invoke-RestMethod -Uri "http://localhost:8080/v1/chat/completions" -Method POST -ContentType "application/json" -Body '{"model":"gpt-4","messages":[{"role":"user","content":"Hello"}]}'
//!
//! # Or with curl (Git Bash)
//! curl -X POST http://localhost:8080/v1/chat/completions -H "Content-Type: application/json" -d '{"model":"gpt-4","messages":[{"role":"user","content":"Hello"}]}'
//! ```

use mofa_foundation::inference::OrchestratorConfig;
use mofa_gateway::server::{GatewayServer, ServerConfig};
use mofa_gateway::GatewayError;
use mofa_runtime::agent::registry::AgentRegistry;
use std::sync::Arc;
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

    info!("Starting MoFA Gateway Server with Inference Bridge");

    // Create agent registry
    let registry = Arc::new(AgentRegistry::new());

    // Create server configuration
    let config = ServerConfig::default()
        .with_host("0.0.0.0")
        .with_port(8080)
        .with_cors(true)
        .with_rate_limit(100, std::time::Duration::from_secs(60));

    // Create orchestrator config for inference bridge
    let orchestrator_config = OrchestratorConfig::default();

    // Create server with inference bridge enabled
    let server = GatewayServer::with_inference(config.clone(), registry, orchestrator_config);

    info!("Gateway server starting on http://0.0.0.0:8080");
    info!("Available endpoints:");
    info!("  GET  /health                    - Health check");
    info!("  GET  /ready                     - Readiness check");
    info!("  POST /v1/chat/completions       - OpenAI chat completions (with inference bridge)");

    // Build and start the server
    let router = server.build_router();
    let addr = config.socket_addr();

    let listener = tokio::net::TcpListener::bind(addr).await?;
    info!("Server listening on {}", addr);

    axum::serve(listener, router)
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    Ok(())
}

async fn shutdown_signal() {
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

    #[cfg(unix)]
    tokio::select! {
        _ = ctrl_c => {
            warn!("Shutdown signal received (Ctrl+C)");
        }
        _ = terminate => {
            warn!("Shutdown signal received (terminate)");
        }
    }

    #[cfg(not(unix))]
    ctrl_c.await;

    info!("Shutting down gracefully...");
}
