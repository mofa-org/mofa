//! Control-plane HTTP server

use axum::{Router, http::Method};
use mofa_foundation::inference::OrchestratorConfig;
use mofa_foundation::rag::embedding_adapter::{RagEmbeddingConfig, RagEmbeddingProvider};
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::TraceLayer;
use tracing::info;

use crate::handlers::{agents_router, chat_router, health_router, openai_router};
use crate::inference_bridge::InferenceBridge;
use crate::middleware::{RateLimiter, SemanticCache, SemanticCacheConfig};
use crate::state::AppState;
use mofa_runtime::agent::registry::AgentRegistry;

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
    /// Enable semantic cache lookups and writes
    pub semantic_cache_enabled: bool,
    /// Similarity score threshold required for a semantic cache hit
    pub semantic_cache_threshold: f32,
    /// Number of nearest entries to inspect in semantic cache
    pub semantic_cache_top_k: usize,
    /// Embedding provider used for semantic cache (`openai` or `ollama`)
    pub semantic_cache_embedding_provider: RagEmbeddingProvider,
    /// Optional embedding model override used for semantic cache
    pub semantic_cache_embedding_model: Option<String>,
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
            semantic_cache_enabled: false,
            semantic_cache_threshold: 0.95,
            semantic_cache_top_k: 8,
            semantic_cache_embedding_provider: RagEmbeddingProvider::OpenAi,
            semantic_cache_embedding_model: None,
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

    /// Configure semantic cache behavior and embedding backend.
    pub fn with_semantic_cache(
        mut self,
        enabled: bool,
        threshold: f32,
        top_k: usize,
        provider: RagEmbeddingProvider,
        model: Option<String>,
    ) -> Self {
        self.semantic_cache_enabled = enabled;
        self.semantic_cache_threshold = threshold;
        self.semantic_cache_top_k = top_k;
        self.semantic_cache_embedding_provider = provider;
        self.semantic_cache_embedding_model = model;
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
}

impl GatewayServer {
    /// Create a server backed by the given `AgentRegistry`.
    pub fn new(config: ServerConfig, registry: Arc<AgentRegistry>) -> Self {
        Self {
            config,
            registry,
            orchestrator_config: None,
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
        }
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
        let semantic_cache = Arc::new(SemanticCache::from_config(SemanticCacheConfig {
            enabled: self.config.semantic_cache_enabled,
            similarity_threshold: self.config.semantic_cache_threshold,
            search_top_k: self.config.semantic_cache_top_k,
            embedding: RagEmbeddingConfig {
                provider: self.config.semantic_cache_embedding_provider,
                model: self.config.semantic_cache_embedding_model.clone(),
                ..RagEmbeddingConfig::default()
            },
        }));

        let state = Arc::new(AppState::new(
            self.registry.clone(),
            rate_limiter.clone(),
            semantic_cache,
        ));

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
                .allow_methods([Method::GET, Method::POST, Method::DELETE, Method::OPTIONS])
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
    fn semantic_cache_builder_methods() {
        let cfg = ServerConfig::new().with_semantic_cache(
            true,
            0.97,
            10,
            RagEmbeddingProvider::Ollama,
            Some("nomic-embed-text".to_string()),
        );

        assert!(cfg.semantic_cache_enabled);
        assert_eq!(cfg.semantic_cache_threshold, 0.97);
        assert_eq!(cfg.semantic_cache_top_k, 10);
        assert_eq!(cfg.semantic_cache_embedding_provider, RagEmbeddingProvider::Ollama);
        assert_eq!(
            cfg.semantic_cache_embedding_model,
            Some("nomic-embed-text".to_string())
        );
    }

    #[test]
    fn socket_addr_parses() {
        let cfg = ServerConfig::new().with_host("127.0.0.1").with_port(8090);
        let addr = cfg.socket_addr();
        assert_eq!(addr.port(), 8090);
    }
}
