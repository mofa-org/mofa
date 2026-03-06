//! Gateway HTTP Server Example
//!
//! This example demonstrates how to start a gateway HTTP server that provides:
//! - Health check endpoints (`/health`, `/ready`)
//! - Agent registry API (`/api/v1/agents`)
//! - Cluster status API (`/api/v1/cluster/status`)
//! - Request routing (`/api/v1/route`)
//!
//! # Usage
//!
//! ```bash
//! cargo run --example gateway_server --no-default-features
//! ```
//!
//! Then test the endpoints:
//! ```bash
//! curl http://localhost:8080/health
//! curl http://localhost:8080/ready
//! curl http://localhost:8080/api/v1/cluster/status
//! ```

use mofa_gateway::gateway::{Gateway, GatewayConfig};
use mofa_gateway::types::LoadBalancingAlgorithm;
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

    info!("Starting MoFA Gateway Server");

    // Create gateway configuration
    let config = GatewayConfig {
        listen_addr: "0.0.0.0:8080".parse().unwrap(),
        load_balancing: LoadBalancingAlgorithm::RoundRobin,
        enable_rate_limiting: true,
        enable_circuit_breakers: true,
    };

    // Create gateway instance (without control plane for this example)
    let mut gateway = Gateway::new(config).await?;

    // Start the gateway server
    gateway.start().await?;

    info!("Gateway server started on http://0.0.0.0:8080");
    info!("Available endpoints:");
    info!("  GET  /health                    - Health check");
    info!("  GET  /ready                     - Readiness check");
    info!("  GET  /api/v1/agents              - List agents");
    info!("  GET  /api/v1/agents/:agent_id    - Get agent info");
    info!("  DELETE /api/v1/agents/:agent_id  - Unregister agent");
    info!("  GET  /api/v1/cluster/nodes       - List cluster nodes");
    info!("  GET  /api/v1/cluster/status      - Cluster status");
    info!("  POST /api/v1/route               - Route a request");

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
    }

    // Stop the gateway
    gateway.stop().await?;
    info!("Gateway server stopped");

    Ok(())
}
