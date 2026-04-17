//! Prometheus metrics endpoint handler for the control-plane server.
//!
//! Exposes `GET /metrics` in Prometheus text-exposition format so that
//! Prometheus (or any compatible scraper) can collect control-plane metrics.

use axum::{extract::State, http::StatusCode, response::IntoResponse};
use std::sync::Arc;

use crate::state::AppState;

/// `GET /metrics` — Prometheus scrape endpoint.
///
/// Returns all registered metrics from [`crate::observability::GatewayMetrics`]
/// in Prometheus text format (`text/plain; version=0.0.4; charset=utf-8`).
pub async fn metrics_handler(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    match state.metrics.export() {
        Ok(body) => (
            StatusCode::OK,
            [(
                axum::http::header::CONTENT_TYPE,
                "text/plain; version=0.0.4; charset=utf-8",
            )],
            body,
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to export metrics: {e}"),
        )
            .into_response(),
    }
}

/// Build the metrics router sub-tree.
pub fn metrics_router() -> axum::Router<Arc<AppState>> {
    use axum::routing::get;
    axum::Router::new().route("/metrics", get(metrics_handler))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::middleware::RateLimiter;
    use crate::observability::GatewayMetrics;
    use axum::body::Body;
    use axum::extract::Request;
    use mofa_runtime::agent::registry::AgentRegistry;
    use std::time::Duration;
    use tower::ServiceExt;

    #[tokio::test]
    async fn metrics_endpoint_returns_prometheus_format() {
        let registry = Arc::new(AgentRegistry::new());
        let rate_limiter = Arc::new(RateLimiter::new(100, Duration::from_secs(60)));
        let metrics = Arc::new(GatewayMetrics::new());

        // Record some metrics to verify they appear in output.
        metrics.increment_requests();
        metrics.increment_requests();
        metrics.increment_errors();

        let state = Arc::new(AppState::new(registry, rate_limiter, metrics));

        let app = metrics_router().with_state(state);

        let req: Request<Body> = Request::builder()
            .uri("/metrics")
            .body(Body::empty())
            .unwrap();

        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let content_type = resp
            .headers()
            .get("content-type")
            .unwrap()
            .to_str()
            .unwrap();
        assert!(content_type.contains("text/plain"));

        let body = axum::body::to_bytes(resp.into_body(), 1024 * 1024)
            .await
            .unwrap();
        let body_str = String::from_utf8_lossy(&body);

        assert!(
            body_str.contains("gateway_requests_total"),
            "should contain requests_total metric"
        );
        assert!(
            body_str.contains("gateway_requests_errors_total"),
            "should contain errors_total metric"
        );
        assert!(
            body_str.contains("gateway_requests_duration_seconds"),
            "should contain duration metric"
        );
    }
}
