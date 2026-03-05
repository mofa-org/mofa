//! Control-plane HTTP server

use axum::{Router, http::Method};
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::TraceLayer;
use tracing::info;

use crate::handlers::{agents_router, chat_router, health_router};
use crate::middleware::RateLimiter;
use crate::state::AppState;
use mofa_runtime::agent::registry::AgentRegistry;

/// Control-plane server configuration
#[derive(Debug, Clone)]
pub struct GatewayConfig {
    /// Bind host
    pub host: String,
    /// Bind port
    pub port: u16,
    /// Whether to enable CORS for all origins
    pub enable_cors: bool,
    /// Whether to enable per-request tracing logs
    pub enable_tracing: bool,
    /// Maximum requests allowed per client per `rate_window`
    pub rate_max_requests: u64,
    /// Time window for the rate limiter
    pub rate_window: Duration,
}

impl Default for GatewayConfig {
    fn default() -> Self {
        Self {
            host: "0.0.0.0".to_string(),
            port: 8090,
            enable_cors: true,
            enable_tracing: true,
            rate_max_requests: 100,
            rate_window: Duration::from_secs(60),
        }
    }
}

impl GatewayConfig {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_host(mut self, host: impl Into<String>) -> Self {
        self.host = host.into();
        self
    }

    pub fn with_port(mut self, port: u16) -> Self {
        self.port = port;
        self
    }

    pub fn with_cors(mut self, enable: bool) -> Self {
        self.enable_cors = enable;
        self
    }

    pub fn with_rate_limit(mut self, max_requests: u64, window: Duration) -> Self {
        self.rate_max_requests = max_requests;
        self.rate_window = window;
        self
    }

    pub fn socket_addr(&self) -> SocketAddr {
        format!("{}:{}", self.host, self.port)
            .parse()
            .unwrap_or_else(|_| SocketAddr::from(([0, 0, 0, 0], self.port)))
    }
}

/// Control-plane server that exposes the agent management REST API
pub struct GatewayServer {
    config: GatewayConfig,
    registry: Arc<AgentRegistry>,
}

impl GatewayServer {
    /// Create a server backed by the given `AgentRegistry`.
    pub fn new(config: GatewayConfig, registry: Arc<AgentRegistry>) -> Self {
        Self { config, registry }
    }

    /// Build the axum `Router` without starting the server.
    ///
    /// Useful for integration tests that want to drive the server via
    /// `axum::serve` or `tower::ServiceExt`.
    pub fn build_router(&self) -> Router {
        let rate_limiter = Arc::new(RateLimiter::new(
            self.config.rate_max_requests,
            self.config.rate_window,
        ));

        let state = Arc::new(AppState::new(self.registry.clone(), rate_limiter.clone()));

        // Spawn background GC task for rate-limiter entries
        let gc_limiter = rate_limiter.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(120));
            loop {
                interval.tick().await;
                gc_limiter.gc();
            }
        });

        let mut router = Router::new()
            .merge(health_router())
            .merge(agents_router())
            .merge(chat_router())
            .with_state(state);

        if self.config.enable_tracing {
            router = router.layer(TraceLayer::new_for_http());
        }

        if self.config.enable_cors {
            let cors = CorsLayer::new()
                .allow_origin(Any)
                .allow_methods([
                    Method::GET,
                    Method::POST,
                    Method::DELETE,
                    Method::OPTIONS,
                ])
                .allow_headers(Any);
            router = router.layer(cors);
        }

        router
    }

    /// Start the server and block until it exits.
    pub async fn start(self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let addr = self.config.socket_addr();
        info!("MoFA control-plane starting on http://{}", addr);

        let router = self.build_router();
        let listener = tokio::net::TcpListener::bind(addr).await?;
        axum::serve(listener, router).await?;
        Ok(())
    }

    /// Start the server in a background Tokio task.
    pub fn start_background(
        self,
    ) -> tokio::task::JoinHandle<Result<(), Box<dyn std::error::Error + Send + Sync>>> {
        tokio::spawn(async move { self.start().await })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config() {
        let cfg = GatewayConfig::default();
        assert_eq!(cfg.port, 8090);
        assert!(cfg.enable_cors);
    }

    #[test]
    fn builder_methods() {
        let cfg = GatewayConfig::new()
            .with_host("127.0.0.1")
            .with_port(9000)
            .with_cors(false)
            .with_rate_limit(50, Duration::from_secs(30));

        assert_eq!(cfg.host, "127.0.0.1");
        assert_eq!(cfg.port, 9000);
        assert!(!cfg.enable_cors);
        assert_eq!(cfg.rate_max_requests, 50);
    }

    #[test]
    fn socket_addr_parses() {
        let cfg = GatewayConfig::new().with_host("127.0.0.1").with_port(8090);
        let addr = cfg.socket_addr();
        assert_eq!(addr.port(), 8090);
    }
}
