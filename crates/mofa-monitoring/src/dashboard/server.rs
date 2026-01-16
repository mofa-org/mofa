//! Dashboard web server
//!
//! Main server component that serves the dashboard

use axum::{
    Router,
    extract::Path,
    http::{Method, StatusCode, header},
    response::IntoResponse,
    routing::get,
};
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
use tower::ServiceBuilder;
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::TraceLayer;
use tracing::info;

use super::api::create_api_router;
use super::assets::{INDEX_HTML, serve_asset};
use super::metrics::{MetricsCollector, MetricsConfig};
use super::websocket::{WebSocketHandler, create_websocket_handler};

/// Dashboard server configuration
#[derive(Debug, Clone)]
pub struct DashboardConfig {
    /// Server host
    pub host: String,
    /// Server port
    pub port: u16,
    /// Enable CORS
    pub enable_cors: bool,
    /// Metrics configuration
    pub metrics_config: MetricsConfig,
    /// WebSocket update interval
    pub ws_update_interval: Duration,
    /// Enable request tracing
    pub enable_tracing: bool,
}

impl Default for DashboardConfig {
    fn default() -> Self {
        Self {
            host: "0.0.0.0".to_string(),
            port: 8080,
            enable_cors: true,
            metrics_config: MetricsConfig::default(),
            ws_update_interval: Duration::from_secs(1),
            enable_tracing: true,
        }
    }
}

impl DashboardConfig {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_host(mut self, host: &str) -> Self {
        self.host = host.to_string();
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

    pub fn with_metrics_config(mut self, config: MetricsConfig) -> Self {
        self.metrics_config = config;
        self
    }

    pub fn with_ws_interval(mut self, interval: Duration) -> Self {
        self.ws_update_interval = interval;
        self
    }

    pub fn socket_addr(&self) -> SocketAddr {
        format!("{}:{}", self.host, self.port)
            .parse()
            .unwrap_or_else(|_| SocketAddr::from(([0, 0, 0, 0], self.port)))
    }
}

/// Server state shared across handlers
pub struct ServerState {
    pub collector: Arc<MetricsCollector>,
    pub ws_handler: Arc<WebSocketHandler>,
}

/// Dashboard server
pub struct DashboardServer {
    config: DashboardConfig,
    collector: Arc<MetricsCollector>,
    ws_handler: Option<Arc<WebSocketHandler>>,
}

impl DashboardServer {
    /// Create a new dashboard server
    pub fn new(config: DashboardConfig) -> Self {
        let collector = Arc::new(MetricsCollector::new(config.metrics_config.clone()));

        Self {
            config,
            collector,
            ws_handler: None,
        }
    }

    /// Get the metrics collector
    pub fn collector(&self) -> Arc<MetricsCollector> {
        self.collector.clone()
    }

    /// Get the WebSocket handler (if started)
    pub fn ws_handler(&self) -> Option<Arc<WebSocketHandler>> {
        self.ws_handler.clone()
    }

    /// Build the router
    pub fn build_router(&mut self) -> Router {
        // Create WebSocket handler
        let (ws_handler, ws_route) = create_websocket_handler(self.collector.clone());
        self.ws_handler = Some(ws_handler.clone());

        // API routes
        let api_router = create_api_router(self.collector.clone());

        // Build main router
        let mut router = Router::new()
            // Static assets
            .route("/", get(serve_index))
            .route("/index.html", get(serve_index))
            .route("/styles.css", get(serve_styles))
            .route("/app.js", get(serve_app_js))
            .route("/assets/*path", get(serve_static))
            // API routes
            .nest("/api", api_router)
            // WebSocket
            .route("/ws", ws_route);

        // Add middleware
        let _service_builder = ServiceBuilder::new();

        if self.config.enable_tracing {
            router = router.layer(TraceLayer::new_for_http());
        }

        if self.config.enable_cors {
            let cors = CorsLayer::new()
                .allow_origin(Any)
                .allow_methods([Method::GET, Method::POST, Method::OPTIONS])
                .allow_headers(Any);
            router = router.layer(cors);
        }

        router
    }

    /// Start the server
    pub async fn start(mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let addr = self.config.socket_addr();
        let _ws_interval = self.config.ws_update_interval;

        info!("Building dashboard server...");
        let router = self.build_router();

        // Start metrics collection
        let collector = self.collector.clone();
        tokio::spawn(async move {
            collector.start_collection();
        });

        // Start WebSocket updates
        if let Some(ws_handler) = &self.ws_handler {
            let _handler = ws_handler.clone();
            tokio::spawn(async move {
                let handler = Arc::new(WebSocketHandler::new(Arc::new(MetricsCollector::new(
                    MetricsConfig::default(),
                ))));
                handler.start_updates();
            });
        }

        info!("Starting dashboard server on {}", addr);
        info!("Dashboard URL: http://{}", addr);

        let listener = tokio::net::TcpListener::bind(addr).await?;
        axum::serve(listener, router).await?;

        Ok(())
    }

    /// Start the server in background
    pub fn start_background(
        self,
    ) -> tokio::task::JoinHandle<Result<(), Box<dyn std::error::Error + Send + Sync>>> {
        tokio::spawn(async move { self.start().await })
    }
}

/// Serve index.html
async fn serve_index() -> impl IntoResponse {
    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, "text/html")],
        INDEX_HTML,
    )
}

/// Serve styles.css
async fn serve_styles() -> impl IntoResponse {
    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, "text/css")],
        super::assets::STYLES_CSS,
    )
}

/// Serve app.js
async fn serve_app_js() -> impl IntoResponse {
    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, "application/javascript")],
        super::assets::APP_JS,
    )
}

/// Serve static assets
async fn serve_static(Path(path): Path<String>) -> impl IntoResponse {
    serve_asset(path).await
}

/// Create a simple dashboard server with default configuration
pub fn create_dashboard(port: u16) -> DashboardServer {
    let config = DashboardConfig::new().with_port(port);
    DashboardServer::new(config)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dashboard_config_default() {
        let config = DashboardConfig::default();
        assert_eq!(config.port, 8080);
        assert!(config.enable_cors);
        assert!(config.enable_tracing);
    }

    #[test]
    fn test_dashboard_config_builder() {
        let config = DashboardConfig::new()
            .with_host("127.0.0.1")
            .with_port(3000)
            .with_cors(false);

        assert_eq!(config.host, "127.0.0.1");
        assert_eq!(config.port, 3000);
        assert!(!config.enable_cors);
    }

    #[test]
    fn test_socket_addr() {
        let config = DashboardConfig::new()
            .with_host("127.0.0.1")
            .with_port(8080);

        let addr = config.socket_addr();
        assert_eq!(addr.port(), 8080);
    }

    #[tokio::test]
    async fn test_dashboard_server_new() {
        let config = DashboardConfig::default();
        let server = DashboardServer::new(config);

        assert!(server.ws_handler.is_none());
    }
}
