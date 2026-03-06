//! Admin HTTP server bound on a separate port.

use std::net::SocketAddr;
use std::sync::Arc;

use axum::Router;
use tracing::info;

use super::handlers::admin_router;
use super::state::AdminState;

/// Configuration for the admin server.
#[derive(Debug, Clone)]
pub struct AdminServerConfig {
    /// Bind host.
    pub host: String,
    /// Admin port (default 9090, separate from the main gateway port).
    pub port: u16,
    /// Admin API key required in `X-Admin-Key` header.
    pub admin_key: String,
}

impl Default for AdminServerConfig {
    fn default() -> Self {
        Self {
            host: "127.0.0.1".to_string(),
            port: 9090,
            admin_key: "changeme".to_string(),
        }
    }
}

impl AdminServerConfig {
    pub fn new(admin_key: impl Into<String>) -> Self {
        Self {
            admin_key: admin_key.into(),
            ..Default::default()
        }
    }

    pub fn with_port(mut self, port: u16) -> Self {
        self.port = port;
        self
    }

    pub fn socket_addr(&self) -> SocketAddr {
        format!("{}:{}", self.host, self.port)
            .parse()
            .unwrap_or_else(|_| SocketAddr::from(([127, 0, 0, 1], self.port)))
    }
}

/// Admin REST API server bound on a dedicated port.
pub struct AdminServer {
    pub config: AdminServerConfig,
    pub state: Arc<AdminState>,
}

impl AdminServer {
    pub fn new(config: AdminServerConfig) -> Self {
        let state = Arc::new(AdminState::new(config.admin_key.clone()));
        Self { config, state }
    }

    /// Build the axum router without binding.  Useful for in-process tests.
    pub fn build_router(&self) -> Router {
        admin_router().with_state(Arc::clone(&self.state))
    }

    /// Start the admin server and block until it exits.
    pub async fn start(self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let addr = self.config.socket_addr();
        info!("MoFA admin API starting on http://{}", addr);
        let router = self.build_router();
        let listener = tokio::net::TcpListener::bind(addr).await?;
        axum::serve(listener, router).await?;
        Ok(())
    }
}
