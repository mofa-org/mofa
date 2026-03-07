use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use axum::{Router, http::Method};
use tower::BoxError;
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::TraceLayer;
use tracing::info;

use super::state::ControlPlaneState;

// Compile-time assertion to ensure ControlPlaneState is Send + Sync + 'static.
// This guarantees the state can be safely shared across async tasks in Axum.
const _: () = {
    fn assert_state_bounds<T: Send + Sync + 'static>() {}
    #[allow(dead_code)]
    fn check() {
        assert_state_bounds::<ControlPlaneState>();
    }
};

#[allow(dead_code)]
fn assert_router_state_is_thread_safe() {
    fn assert_bounds<T: Clone + Send + Sync + 'static>() {}
    assert_bounds::<Arc<ControlPlaneState>>();
}

/// Configuration for the control plane HTTP server.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ControlPlaneConfig {
    /// Bind host (default: 0.0.0.0).
    pub host: String,
    /// Bind port (default: 8081 to avoid clashing with monitoring).
    pub port: u16,
    /// Enable permissive CORS for development.
    pub enable_cors: bool,
    /// Enable request tracing via `tower_http::trace`.
    pub enable_tracing: bool,
    /// Graceful shutdown timeout to reuse in later phases.
    pub shutdown_grace_period: Duration,
}

impl Default for ControlPlaneConfig {
    fn default() -> Self {
        Self {
            host: "0.0.0.0".to_string(),
            port: 8081,
            enable_cors: true,
            enable_tracing: true,
            shutdown_grace_period: Duration::from_secs(5),
        }
    }
}

impl ControlPlaneConfig {
    /// Create a new config with default values.
    pub fn new() -> Self {
        Self::default()
    }

    /// Override the bind host.
    pub fn with_host(mut self, host: impl Into<String>) -> Self {
        self.host = host.into();
        self
    }

    /// Override the bind port.
    pub fn with_port(mut self, port: u16) -> Self {
        self.port = port;
        self
    }

    /// Enable or disable CORS.
    pub fn with_cors(mut self, enable: bool) -> Self {
        self.enable_cors = enable;
        self
    }

    /// Enable or disable tracing middleware.
    pub fn with_tracing(mut self, enable: bool) -> Self {
        self.enable_tracing = enable;
        self
    }

    /// Set the graceful shutdown grace period.
    pub fn with_shutdown_grace(mut self, grace: Duration) -> Self {
        self.shutdown_grace_period = grace;
        self
    }

    /// Resolve the configured socket address.
    pub fn socket_addr(&self) -> SocketAddr {
        format!("{}:{}", self.host, self.port)
            .parse()
            .unwrap_or_else(|_| SocketAddr::from(([0, 0, 0, 0], self.port)))
    }
}

/// HTTP server that fronts the control plane and gateway.
#[derive(Clone)]
pub struct ControlPlaneServer {
    config: ControlPlaneConfig,
    state: Arc<ControlPlaneState>,
}

impl ControlPlaneServer {
    /// Create a new server from config and shared state.
    pub fn new(config: ControlPlaneConfig, state: Arc<ControlPlaneState>) -> Self {
        Self { config, state }
    }

    /// Access the server configuration.
    pub fn config(&self) -> &ControlPlaneConfig {
        &self.config
    }

    /// Access the shared state.
    pub fn state(&self) -> Arc<ControlPlaneState> {
        self.state.clone()
    }

    /// Build the axum router with shared state and middleware layers.
    pub fn build_router(&self) -> Router {
        let mut router: Router<Arc<ControlPlaneState>> = Router::new();

        if self.config.enable_tracing {
            router = router.layer(TraceLayer::new_for_http());
        }

        if self.config.enable_cors {
            let cors = CorsLayer::new()
                .allow_origin(Any)
                .allow_methods([
                    Method::GET,
                    Method::POST,
                    Method::PUT,
                    Method::DELETE,
                    Method::OPTIONS,
                ])
                .allow_headers(Any);
            router = router.layer(cors);
        }

        router.with_state(self.state.clone())
    }

    /// Start the server and block until shutdown.
    pub async fn start(self) -> ControlPlaneServerResult<()> {
        let addr = self.config.socket_addr();
        info!("Starting MoFA HTTP control plane on http://{}", addr);

        let listener = tokio::net::TcpListener::bind(addr)
            .await
            .map_err(|source| ControlPlaneServerError::Bind { addr, source })?;

        let router = self.build_router();

        axum::serve(listener, router)
            .with_graceful_shutdown(async {
                // Placeholder for shutdown signals; keeping the hook enables later phases
                // to install signal handling without changing the server interface.
                tokio::signal::ctrl_c().await.unwrap_or_else(|_| ());
            })
            .await
            .map_err(|err| ControlPlaneServerError::Serve(err.into()))?;

        Ok(())
    }
}

/// Error type for the control plane server lifecycle.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum ControlPlaneServerError {
    /// Binding the TCP listener failed.
    #[error("failed to bind control plane listener on {addr}")]
    Bind {
        addr: SocketAddr,
        #[source]
        source: std::io::Error,
    },
    /// axum/hyper serving error.
    #[error("control plane server failed")]
    Serve(#[source] BoxError),
}

/// Convenient result alias for control plane server operations.
pub type ControlPlaneServerResult<T> = Result<T, ControlPlaneServerError>;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::{AgentRegistry, ExecutionEngine};
    use axum::{body::Body, http::Request};
    use tower::ServiceExt;

    fn make_state() -> Arc<ControlPlaneState> {
        let registry = Arc::new(AgentRegistry::new());
        let engine = Arc::new(ExecutionEngine::new(registry.clone()));
        Arc::new(ControlPlaneState::new(registry, engine))
    }

    #[test]
    fn config_defaults_are_sane() {
        let cfg = ControlPlaneConfig::default();
        assert_eq!(cfg.host, "0.0.0.0");
        assert_eq!(cfg.port, 8081);
        assert!(cfg.enable_cors);
        assert!(cfg.enable_tracing);
        assert_eq!(cfg.shutdown_grace_period, Duration::from_secs(5));
    }

    #[test]
    fn config_builder_methods_override_fields() {
        let cfg = ControlPlaneConfig::new()
            .with_host("127.0.0.1")
            .with_port(9090)
            .with_cors(false)
            .with_tracing(false)
            .with_shutdown_grace(Duration::from_secs(2));

        assert_eq!(cfg.host, "127.0.0.1");
        assert_eq!(cfg.port, 9090);
        assert!(!cfg.enable_cors);
        assert!(!cfg.enable_tracing);
        assert_eq!(cfg.shutdown_grace_period, Duration::from_secs(2));
    }

    #[test]
    fn socket_addr_parses() {
        let cfg = ControlPlaneConfig::new()
            .with_host("127.0.0.1")
            .with_port(8081);
        let addr = cfg.socket_addr();
        assert_eq!(addr.port(), 8081);
        assert_eq!(addr.ip().to_string(), "127.0.0.1");
    }

    #[tokio::test]
    async fn build_router_initializes_state_and_layers() {
        let state = make_state();
        let server = ControlPlaneServer::new(ControlPlaneConfig::default(), state.clone());
        let router = server.build_router();

        let mut service = router.into_service();

        let response = service
            .oneshot(
                Request::builder()
                    .uri("/")
                    .body(Body::empty())
                    .expect("failed to build request"),
            )
            .await
            .expect("router service error");

        assert_eq!(response.status(), axum::http::StatusCode::NOT_FOUND);

        // Ensure state is still accessible after building the router.
        let cloned_state = server.state();
        assert!(Arc::ptr_eq(&state, &cloned_state));
    }
}
