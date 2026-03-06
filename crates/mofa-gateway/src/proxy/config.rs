//! Proxy backend configuration.

use std::time::Duration;

/// Configuration for a proxy backend service.
#[derive(Debug, Clone)]
pub struct ProxyBackend {
    /// Backend name/identifier.
    pub name: String,
    /// Base URL of the backend service (e.g., "http://localhost:8000").
    pub base_url: String,
    /// Optional health check endpoint path (e.g., "/health").
    pub health_check_endpoint: Option<String>,
    /// Request timeout.
    pub timeout: Duration,
    /// Maximum number of retries on failure (reserved for future implementation).
    /// Note: Retry logic is not currently implemented in ProxyHandler::forward().
    pub retries: u32,
}

impl ProxyBackend {
    /// Create a new proxy backend configuration.
    pub fn new(name: impl Into<String>, base_url: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            base_url: base_url.into(),
            health_check_endpoint: None,
            timeout: Duration::from_secs(60),
            retries: 3,
        }
    }

    /// Set the health check endpoint.
    pub fn with_health_check(mut self, endpoint: impl Into<String>) -> Self {
        self.health_check_endpoint = Some(endpoint.into());
        self
    }

    /// Set the request timeout.
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// Set the maximum number of retries.
    pub fn with_retries(mut self, retries: u32) -> Self {
        self.retries = retries;
        self
    }
}
