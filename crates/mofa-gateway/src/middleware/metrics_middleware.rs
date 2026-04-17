//! Per-request Prometheus metrics middleware for the control-plane server.
//!
//! Instruments every HTTP request with:
//! - `gateway_requests_total` counter (incremented on entry)
//! - `gateway_requests_duration_seconds` histogram (recorded on exit)
//! - `gateway_requests_errors_total` counter (incremented on non-2xx responses)

use axum::{body::Body, extract::Request, middleware::Next, response::Response};
use std::sync::Arc;
use std::time::Instant;

use crate::observability::SharedMetrics;

/// Axum middleware that records per-request Prometheus metrics.
///
/// This middleware must be installed **with state** so it can access the
/// shared [`SharedMetrics`] instance from [`crate::state::AppState`].
///
/// # Metrics updated
///
/// | Metric | Type | When |
/// |--------|------|------|
/// | `gateway_requests_total` | Counter | Every request |
/// | `gateway_requests_duration_seconds` | Histogram | After response |
/// | `gateway_requests_errors_total` | Counter | Non-2xx status |
pub async fn metrics_middleware(
    metrics: axum::extract::Extension<SharedMetrics>,
    req: Request<Body>,
    next: Next,
) -> Response {
    let start = Instant::now();

    // Count every incoming request.
    metrics.increment_requests();

    let response = next.run(req).await;

    // Record latency.
    let duration = start.elapsed();
    metrics.record_request_duration(duration);

    // Count errors (any non-2xx status).
    if !response.status().is_success() {
        metrics.increment_errors();
    }

    response
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::observability::GatewayMetrics;
    use axum::http::StatusCode;
    use axum::{Router, body::Body, response::IntoResponse, routing::get};
    use tower::ServiceExt;

    async fn ok_handler() -> &'static str {
        "ok"
    }

    async fn error_handler() -> impl IntoResponse {
        (StatusCode::INTERNAL_SERVER_ERROR, "error")
    }

    fn build_router(metrics: SharedMetrics) -> Router {
        Router::new()
            .route("/ok", get(ok_handler))
            .route("/err", get(error_handler))
            .layer(axum::middleware::from_fn(metrics_middleware))
            .layer(axum::Extension(metrics))
    }

    #[tokio::test]
    async fn increments_requests_on_success() {
        let metrics = Arc::new(GatewayMetrics::new());
        let app = build_router(metrics.clone());

        let req = Request::builder().uri("/ok").body(Body::empty()).unwrap();

        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        assert_eq!(metrics.requests_total.get() as u64, 1);
    }

    #[tokio::test]
    async fn increments_errors_on_non_2xx() {
        let metrics = Arc::new(GatewayMetrics::new());
        let app = build_router(metrics.clone());

        let req = Request::builder().uri("/err").body(Body::empty()).unwrap();

        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(metrics.requests_total.get() as u64, 1);
        assert_eq!(metrics.requests_errors_total.get() as u64, 1);
    }
}
