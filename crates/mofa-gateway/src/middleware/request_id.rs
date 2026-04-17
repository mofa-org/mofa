//! Request-ID middleware for the control-plane server.
//!
//! Ensures every request/response carries an `X-Request-Id` header for
//! end-to-end correlation across logs, tracing spans, and downstream services.

use axum::{
    body::Body,
    extract::Request,
    http::{HeaderName, HeaderValue},
    middleware::Next,
    response::Response,
};

/// Extension type that handlers can extract to obtain the current request ID.
#[derive(Debug, Clone)]
pub struct RequestId(pub String);

/// Header name used for request-ID propagation.
static REQUEST_ID_HEADER: HeaderName = HeaderName::from_static("x-request-id");

/// Axum middleware that propagates or generates an `X-Request-Id`.
///
/// Behaviour:
/// 1. If the incoming request carries an `X-Request-Id` header, its value is
///    reused verbatim.
/// 2. Otherwise a new UUID v4 is generated.
/// 3. The resolved ID is stored as a [`RequestId`] request extension so
///    downstream handlers (and tracing subscribers) can read it.
/// 4. The same ID is injected into the *response* `X-Request-Id` header.
pub async fn request_id_middleware(mut req: Request<Body>, next: Next) -> Response {
    // Resolve: use the client-provided ID or generate a new one.
    let id = req
        .headers()
        .get(&REQUEST_ID_HEADER)
        .and_then(|v| v.to_str().ok())
        .map(String::from)
        .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());

    // Make the ID available to handlers via request extensions.
    req.extensions_mut().insert(RequestId(id.clone()));

    // Add a structured field to the current tracing span.
    tracing::Span::current().record("request_id", &id.as_str());

    let mut response = next.run(req).await;

    // Echo the resolved ID back to the caller.
    if let Ok(value) = HeaderValue::from_str(&id) {
        response
            .headers_mut()
            .insert(REQUEST_ID_HEADER.clone(), value);
    }

    response
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::StatusCode;
    use axum::{Router, body::Body, routing::get};
    use tower::ServiceExt;

    async fn echo_handler() -> &'static str {
        "ok"
    }

    fn test_router() -> Router {
        Router::new()
            .route("/ping", get(echo_handler))
            .layer(axum::middleware::from_fn(request_id_middleware))
    }

    #[tokio::test]
    async fn generates_request_id_when_absent() {
        let app = test_router();

        let req = Request::builder().uri("/ping").body(Body::empty()).unwrap();

        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let header = resp.headers().get("x-request-id");
        assert!(header.is_some(), "response must contain X-Request-Id");
        // UUID v4 is 36 chars (8-4-4-4-12)
        assert_eq!(header.unwrap().len(), 36);
    }

    #[tokio::test]
    async fn preserves_client_provided_request_id() {
        let app = test_router();

        let req = Request::builder()
            .uri("/ping")
            .header("x-request-id", "my-custom-id-123")
            .body(Body::empty())
            .unwrap();

        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let header = resp.headers().get("x-request-id").unwrap();
        assert_eq!(header.to_str().unwrap(), "my-custom-id-123");
    }
}
