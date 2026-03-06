//! mofa-local-llm backend configuration.

use super::config::ProxyBackend;
use std::time::Duration;

/// Configuration for mofa-local-llm backend.
#[derive(Debug, Clone)]
pub struct LocalLLMBackend {
    /// Base URL of mofa-local-llm HTTP server.
    pub base_url: String,
    /// Health check endpoint path.
    pub health_endpoint: String,
    /// Request timeout.
    pub timeout: Duration,
}

impl LocalLLMBackend {
    /// Create a default LocalLLMBackend configuration.
    ///
    /// Reads `MOFA_LOCAL_LLM_URL` environment variable or defaults to
    /// `http://localhost:8000`.
    pub fn default() -> Self {
        let base_url = std::env::var("MOFA_LOCAL_LLM_URL")
            .unwrap_or_else(|_| "http://localhost:8000".to_string());

        Self {
            base_url: base_url.trim_end_matches('/').to_string(),
            health_endpoint: "/health".to_string(),
            timeout: Duration::from_secs(60),
        }
    }

    /// Create from explicit base URL.
    pub fn new(base_url: impl Into<String>) -> Self {
        Self {
            base_url: base_url.into().trim_end_matches('/').to_string(),
            health_endpoint: "/health".to_string(),
            timeout: Duration::from_secs(60),
        }
    }

    /// Set the health check endpoint.
    pub fn with_health_endpoint(mut self, endpoint: impl Into<String>) -> Self {
        self.health_endpoint = endpoint.into();
        self
    }

    /// Set the request timeout.
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// Convert to ProxyBackend for use with ProxyHandler.
    pub fn to_proxy_backend(&self) -> ProxyBackend {
        ProxyBackend::new("mofa-local-llm", &self.base_url)
            .with_health_check(&self.health_endpoint)
            .with_timeout(self.timeout)
    }

    /// Get the full health check URL.
    pub fn health_url(&self) -> String {
        format!("{}{}", self.base_url, self.health_endpoint)
    }

    /// Get the full URL for a given path.
    pub fn url_for(&self, path: &str) -> String {
        let path = path.trim_start_matches('/');
        format!("{}/{}", self.base_url, path)
    }
}
