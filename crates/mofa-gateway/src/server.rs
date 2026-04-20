//! Control-plane HTTP server

use axum::{http::Method, Router};
use mofa_foundation::inference::OrchestratorConfig;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::TraceLayer;
use tracing::info;

use crate::handlers::{agents_router, chat_router, health_router, openai_router};
use crate::inference_bridge::InferenceBridge;
use crate::middleware::RateLimiter;
use crate::state::AppState;
use mofa_kernel::ObjectStore;
use mofa_runtime::agent::registry::AgentRegistry;

#[cfg(feature = "socketio")]
use mofa_integrations::socketio::{SocketIoBridge, SocketIoConfig};
#[cfg(feature = "socketio")]
use mofa_kernel::AgentBus;

#[cfg(feature = "s3")]
use mofa_integrations::s3::S3ObjectStore;
#[cfg(feature = "s3")]
use mofa_integrations::s3::S3Config;

/// Control-plane server configuration
#[derive(Debug, Clone)]
pub struct ServerConfig {
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
    /// Maximum allowed upload body size in bytes.
    ///
    /// Uploads that exceed this are rejected with `413 Payload Too Large`.
    /// `None` means no limit (default).
    pub max_upload_bytes: Option<u64>,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            host: "0.0.0.0".to_string(),
            port: 8090,
            enable_cors: true,
            enable_tracing: true,
            rate_max_requests: 100,
            rate_window: Duration::from_secs(60),
            max_upload_bytes: None,
        }
    }
}

impl ServerConfig {
    /// Create a new `ServerConfig` with default values.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the bind host address.
    pub fn with_host(mut self, host: impl Into<String>) -> Self {
        self.host = host.into();
        self
    }

    /// Set the bind port.
    pub fn with_port(mut self, port: u16) -> Self {
        self.port = port;
        self
    }

    /// Enable or disable CORS for all origins.
    pub fn with_cors(mut self, enable: bool) -> Self {
        self.enable_cors = enable;
        self
    }

    /// Configure the rate limiter: maximum requests per client per window.
    pub fn with_rate_limit(mut self, max_requests: u64, window: Duration) -> Self {
        self.rate_max_requests = max_requests;
        self.rate_window = window;
        self
    }

    /// Set the maximum allowed upload size in bytes.
    ///
    /// Uploads that exceed this limit are rejected with `413 Payload Too Large`.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// // Allow uploads up to 50 MB
    /// ServerConfig::new().with_max_upload_size(50 * 1024 * 1024)
    /// ```
    pub fn with_max_upload_size(mut self, max_bytes: u64) -> Self {
        self.max_upload_bytes = Some(max_bytes);
        self
    }

    /// Return the resolved `SocketAddr` for this configuration.
    pub fn socket_addr(&self) -> SocketAddr {
        format!("{}:{}", self.host, self.port)
            .parse()
            .unwrap_or_else(|_| SocketAddr::from(([0, 0, 0, 0], self.port)))
    }
}

/// Control-plane server that exposes the agent management REST API
pub struct GatewayServer {
    config: ServerConfig,
    registry: Arc<AgentRegistry>,
    /// Optional orchestrator config for inference bridge
    orchestrator_config: Option<OrchestratorConfig>,
    /// Pre-initialised object store injected via `with_s3`.
    s3: Option<Arc<dyn ObjectStore>>,
    #[cfg(feature = "socketio")]
    socket_io: Option<(Arc<AgentBus>, SocketIoConfig)>,
}

impl GatewayServer {
    /// Create a server backed by the given `AgentRegistry`.
    pub fn new(config: ServerConfig, registry: Arc<AgentRegistry>) -> Self {
        Self {
            config,
            registry,
            orchestrator_config: None,
            s3: None,
            #[cfg(feature = "socketio")]
            socket_io: None,
        }
    }

    /// Create a server with inference bridge enabled.
    pub fn with_inference(
        config: ServerConfig,
        registry: Arc<AgentRegistry>,
        orchestrator_config: OrchestratorConfig,
    ) -> Self {
        Self {
            config,
            registry,
            orchestrator_config: Some(orchestrator_config),
            s3: None,
            #[cfg(feature = "socketio")]
            socket_io: None,
        }
    }

    /// Attach a pre-initialised object store for the `/api/v1/files` endpoints.
    ///
    /// Accepts any `Arc<dyn ObjectStore>` — pass an `S3ObjectStore` for AWS/MinIO
    /// or a custom in-memory implementation for testing.
    pub fn with_s3(mut self, store: Arc<dyn ObjectStore>) -> Self {
        self.s3 = Some(store);
        self
    }

    /// Attach an `AgentBus` and `SocketIoConfig` to enable the real-time
    /// Socket.IO bridge.
    ///
    /// Requires the `socketio` feature flag.
    #[cfg(feature = "socketio")]
    pub fn with_socket_io(mut self, bus: Arc<AgentBus>, config: SocketIoConfig) -> Self {
        self.socket_io = Some((bus, config));
        self
    }

    /// Build the axum `Router` without starting the server.
    ///
    /// Useful for integration tests that want to drive the server via
    /// `axum::serve` or `tower::ServiceExt`.
    pub fn build_router(&self) -> Router {
        use crate::handlers::files_router;

        let rate_limiter = Arc::new(RateLimiter::new(
            self.config.rate_max_requests,
            self.config.rate_window,
        ));

        // Create state
        let mut state = AppState::new(self.registry.clone(), rate_limiter.clone());
        // Attach the pre-initialized object store so file handlers can use it.
        // When no store is configured (tests: `build_server_no_s3`) we keep `None`
        // and endpoints correctly return `501 Not Implemented`.
        state.s3 = self.s3.clone();

        if let Some(max_bytes) = self.config.max_upload_bytes {
            state = state.with_max_upload_bytes(max_bytes);
        }

        // Spawn background GC task for rate-limiter entries
        let gc_limiter = rate_limiter.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(120));
            loop {
                interval.tick().await;
                gc_limiter.gc();
            }
        });

        // Build the Socket.IO layer when configured; capture the SocketIo handle
        // so handlers can emit server-side events (e.g. upload progress).
        #[cfg(feature = "socketio")]
        if let Some((bus, sio_cfg)) = &self.socket_io {
            let namespace = sio_cfg.namespace.clone();
            let (sio_layer, _, io) = SocketIoBridge::new(sio_cfg.clone(), bus.clone()).build();
            state = state.with_socketio(io, namespace);
            let state = Arc::new(state);
            let mut router = Router::new()
                .merge(health_router())
                .merge(agents_router())
                .merge(chat_router())
                .merge(files_router())
                .with_state(state);
            router = router.layer(sio_layer);

            // Add OpenAI router if inference bridge is configured
            if let Some(ref orch_config) = self.orchestrator_config {
                let bridge = Arc::new(InferenceBridge::new(orch_config.clone()));
                router = router
                    .merge(openai_router())
                    .layer(axum::Extension(bridge));
            }

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
            return router;
        }

        let state = Arc::new(state);
        let mut router = Router::new()
            .merge(health_router())
            .merge(agents_router())
            .merge(chat_router())
            .merge(files_router())
            .with_state(state);

        // Add OpenAI router if inference bridge is configured
        if let Some(ref orch_config) = self.orchestrator_config {
            let bridge = Arc::new(InferenceBridge::new(orch_config.clone()));
            router = router
                .merge(openai_router())
                .layer(axum::Extension(bridge));
        }

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

/// Convenience constructor: build an [`S3ObjectStore`] from config and wrap it
/// for [`GatewayServer::with_s3`].
///
/// ```rust,no_run
/// # use mofa_gateway::server::{GatewayServer, ServerConfig, make_s3_store};
/// # use mofa_runtime::agent::registry::AgentRegistry;
/// # use std::sync::Arc;
/// #[tokio::main]
/// async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
///     let store = make_s3_store("us-east-1", "my-bucket", None).await?;
///     let server = GatewayServer::new(ServerConfig::default(), Arc::new(AgentRegistry::new()))
///         .with_s3(store);
///     Ok(())
/// }
/// ```
#[cfg(feature = "s3")]
pub async fn make_s3_store(
    region: impl Into<String>,
    bucket: impl Into<String>,
    endpoint: Option<String>,
) -> Result<Arc<dyn ObjectStore>, Box<dyn std::error::Error + Send + Sync>> {
    let mut cfg = S3Config::new(region, bucket);
    if let Some(ep) = endpoint {
        cfg = cfg.with_endpoint(ep);
    }
    let store = S3ObjectStore::new(cfg).await?;
    Ok(Arc::new(store) as Arc<dyn ObjectStore>)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config() {
        let cfg = ServerConfig::default();
        assert_eq!(cfg.port, 8090);
        assert!(cfg.enable_cors);
    }

    #[test]
    fn builder_methods() {
        let cfg = ServerConfig::new()
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
        let cfg = ServerConfig::new().with_host("127.0.0.1").with_port(8090);
        let addr = cfg.socket_addr();
        assert_eq!(addr.port(), 8090);
    }
}
