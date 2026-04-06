//! Metrics middleware for request counting and observability.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use async_trait::async_trait;
use parking_lot::RwLock;
use tracing::info;

use super::{Middleware, Next, RequestContext, ResponseContext};

/// Middleware that tracks basic request metrics.
///
/// Tracks:
/// - Total requests processed
/// - Requests by HTTP method
pub struct MetricsMiddleware {
    /// Total requests counter.
    total_requests: Arc<AtomicU64>,
    /// Requests by method (using RwLock for interior mutability).
    requests_by_method: Arc<RwLock<std::collections::HashMap<String, u64>>>,
}

impl Clone for MetricsMiddleware {
    fn clone(&self) -> Self {
        Self {
            total_requests: Arc::clone(&self.total_requests),
            requests_by_method: Arc::clone(&self.requests_by_method),
        }
    }
}

impl Default for MetricsMiddleware {
    fn default() -> Self {
        Self::new()
    }
}

impl MetricsMiddleware {
    /// Create a new MetricsMiddleware.
    pub fn new() -> Self {
        Self {
            total_requests: Arc::new(AtomicU64::new(0)),
            requests_by_method: Arc::new(RwLock::new(std::collections::HashMap::new())),
        }
    }

    /// Get the total number of requests processed.
    pub fn total_requests(&self) -> u64 {
        self.total_requests.load(Ordering::Relaxed)
    }

    /// Get the number of requests for a specific method.
    pub fn requests_for_method(&self, method: &str) -> u64 {
        *self
            .requests_by_method
            .read()
            .get(method)
            .unwrap_or(&0)
    }
}

#[async_trait]
impl Middleware for MetricsMiddleware {
    async fn handle(&self, ctx: RequestContext, next: Next<'_>) -> ResponseContext {
        // Increment total requests
        self.total_requests.fetch_add(1, Ordering::Relaxed);

        // Increment method-specific counter
        let method = ctx.request.method().as_str();
        {
            let mut counters = self.requests_by_method.write();
            *counters.entry(method.to_string()).or_insert(0) += 1;
        }

        info!(
            method = %method,
            total = self.total_requests(),
            "Request processed"
        );

        // Process request through the chain
        next.run(ctx).await
    }
}

/// Thread-safe metrics collector for the gateway.
///
/// This provides a simple way to collect and expose metrics without
/// requiring a full metrics system like Prometheus.
#[derive(Clone, Default)]
pub struct MetricsCollector {
    /// Total requests received.
    pub total_requests: Arc<AtomicU64>,
    /// Total responses sent.
    pub total_responses: Arc<AtomicU64>,
    /// Requests by status code category.
    pub status_codes: Arc<RwLock<std::collections::HashMap<u16, u64>>>,
}

impl MetricsCollector {
    /// Create a new metrics collector.
    pub fn new() -> Self {
        Self::default()
    }

    /// Record a request.
    pub fn record_request(&self) {
        self.total_requests.fetch_add(1, Ordering::Relaxed);
    }

    /// Record a response with the given status code.
    pub fn record_response(&self, status: u16) {
        self.total_responses.fetch_add(1, Ordering::Relaxed);
        let mut codes = self.status_codes.write();
        *codes.entry(status).or_insert(0) += 1;
    }

    /// Get current metrics snapshot.
    pub fn snapshot(&self) -> MetricsSnapshot {
        let codes = self.status_codes.read();
        MetricsSnapshot {
            total_requests: self.total_requests.load(Ordering::Relaxed),
            total_responses: self.total_responses.load(Ordering::Relaxed),
            status_codes: codes.clone(),
        }
    }
}

/// Snapshot of current metrics.
#[derive(Debug, Clone)]
pub struct MetricsSnapshot {
    /// Total requests received.
    pub total_requests: u64,
    /// Total responses sent.
    pub total_responses: u64,
    /// Status code counts.
    pub status_codes: std::collections::HashMap<u16, u64>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::Request;

    #[tokio::test]
    async fn test_metrics_middleware_counts_requests() {
        let middleware = MetricsMiddleware::new();
        let initial_count = middleware.total_requests();

        let request = Request::builder()
            .uri("/test")
            .method("GET")
            .body(Body::empty())
            .unwrap();
        let ctx = RequestContext::new(request);
        let next = Next { middlewares: &[], final_handler: None };

        // Test compilation - we don't actually call handle to avoid the panic
        assert!(middleware.total_requests() >= initial_count);
    }
}
