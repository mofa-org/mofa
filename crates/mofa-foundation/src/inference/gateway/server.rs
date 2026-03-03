//! Gateway server entrypoint — builds the axum Router and binds to a TCP port.

use std::net::SocketAddr;
use std::sync::{Arc, Mutex};

use axum::Router;
use axum::routing::{get, post};

use crate::inference::orchestrator::InferenceOrchestrator;

use super::handler::{AppState, chat_completions, list_models};
use super::rate_limiter::TokenBucketLimiter;
use super::types::GatewayConfig;

/// The MoFA inference gateway server.
///
/// Wraps an [`InferenceOrchestrator`] behind an OpenAI-compatible HTTP API.
///
/// # Example
///
/// ```rust,no_run
/// use mofa_foundation::inference::gateway::{GatewayConfig, GatewayServer};
///
/// #[tokio::main]
/// async fn main() {
///     let config = GatewayConfig::default().with_port(8080).with_rpm(120);
///     GatewayServer::new(config).serve().await.unwrap();
/// }
/// ```
pub struct GatewayServer {
    config: GatewayConfig,
}

impl GatewayServer {
    /// Create a new server with the given configuration.
    pub fn new(config: GatewayConfig) -> Self {
        Self { config }
    }

    /// Build the axum [`Router`] without binding.
    ///
    /// Useful for integration testing.
    pub fn build_router(&self) -> Router {
        let orchestrator = InferenceOrchestrator::new(self.config.orchestrator_config.clone());
        let state = AppState {
            orchestrator: Arc::new(Mutex::new(orchestrator)),
            limiter: Arc::new(Mutex::new(TokenBucketLimiter::new(
                self.config.rate_limit_rpm,
            ))),
            available_models: self.config.available_models.clone(),
        };

        Router::new()
            .route("/v1/chat/completions", post(chat_completions))
            .route("/v1/models", get(list_models))
            .with_state(state)
    }

    /// Bind to the configured host:port and serve requests.
    ///
    /// Uses `axum::Server` (hyper-backed) which is the axum 0.6 API.
    /// Runs until interrupted.
    pub async fn serve(self) -> Result<(), Box<dyn std::error::Error>> {
        let addr: SocketAddr = format!("{}:{}", self.config.host, self.config.port).parse()?;
        let router = self
            .build_router()
            .into_make_service_with_connect_info::<SocketAddr>();

        tracing::info!("MoFA inference gateway listening on http://{addr}");
        tracing::info!("  POST /v1/chat/completions");
        tracing::info!("  GET  /v1/models");

        axum::Server::bind(&addr).serve(router).await?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_server_builds_router() {
        let config = GatewayConfig::default();
        let server = GatewayServer::new(config);
        // Verify the router builds without panicking
        let _router = server.build_router();
    }

    #[test]
    fn test_config_with_port() {
        let config = GatewayConfig::default().with_port(9090).with_rpm(30);
        assert_eq!(config.port, 9090);
        assert_eq!(config.rate_limit_rpm, 30);
    }
}
