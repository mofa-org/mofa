//! SSE-aware HTTP proxy passthrough.
//!
//! When the upstream backend returns `Content-Type: text/event-stream`,
//! this module forwards the body as a live byte stream without buffering —
//! preserving the SSE semantics end-to-end.
//!
//! For non-streaming responses the full body is collected into memory so
//! `Content-Length` can be set accurately.
//!
//! # Usage (from a proxy handler)
//!
//! ```rust,no_run
//! use mofa_gateway::streaming::proxy::forward_response;
//! use axum::body::Body;
//! use axum::http::Response;
//!
//! // Given a hyper response from an upstream call:
//! // let upstream: hyper::Response<hyper::body::Incoming> = ...;
//! // let axum_resp = forward_response(upstream).await?;
//! ```
//!
//! # Integration with PR #931 (mofa-local-llm proxy)
//!
//! PR #931 adds a `ProxyHandler::forward()` that currently buffers the entire
//! response body with `body.collect().await`. Replace that with a call to
//! [`forward_response`] to enable SSE passthrough.

use axum::body::Body;
use axum::http::{Response, StatusCode};
use http_body_util::BodyExt;
use tracing::{debug, error, warn};

/// Detect whether a response is a Server-Sent Events stream.
///
/// Returns `true` when the `Content-Type` header starts with
/// `text/event-stream`.
pub fn is_sse_response(resp: &Response<hyper::body::Incoming>) -> bool {
    resp.headers()
        .get(axum::http::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .map(|ct| ct.starts_with("text/event-stream"))
        .unwrap_or(false)
}

/// Hop-by-hop headers that must not be forwarded to the client.
///
/// See [RFC 7230 §6.1](https://datatracker.ietf.org/doc/html/rfc7230#section-6.1).
const HOP_BY_HOP: &[&str] = &[
    "connection",
    "keep-alive",
    "proxy-authenticate",
    "proxy-authorization",
    "te",
    "trailers",
    "transfer-encoding",
    "upgrade",
];

/// Forward a hyper upstream response to an axum [`Response<Body>`].
///
/// - **SSE responses** (`Content-Type: text/event-stream`): the body is
///   streamed through without buffering. `Content-Length` is removed because
///   the length is not known in advance.
/// - **Non-SSE responses**: the body is fully buffered so that
///   `Content-Length` can be set accurately.
///
/// Hop-by-hop headers are stripped in both cases.
pub async fn forward_response(
    upstream: Response<hyper::body::Incoming>,
) -> Result<Response<Body>, String> {
    let (parts, body) = upstream.into_parts();

    let streaming = parts
        .headers
        .get(axum::http::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .map(|ct| ct.starts_with("text/event-stream"))
        .unwrap_or(false);

    // Build response, copying headers while stripping hop-by-hop and
    // (for streaming) content-length.
    let mut builder = Response::builder().status(parts.status);

    for (key, value) in &parts.headers {
        let name = key.as_str();
        if HOP_BY_HOP.contains(&name) {
            continue;
        }
        // For streaming responses, content-length is unknown / inapplicable.
        if streaming && name == "content-length" {
            continue;
        }
        builder = builder.header(key.clone(), value.clone());
    }

    if streaming {
        debug!("SSE response detected — streaming body through without buffering");

        // Pass bytes through as a stream — zero buffering.
        let axum_body = Body::from_stream(body.into_data_stream());
        builder
            .body(axum_body)
            .map_err(|e| format!("Failed to build streaming response: {e}"))
    } else {
        // Buffer the full body and set an accurate Content-Length.
        let collected = match body.collect().await {
            Ok(c) => c.to_bytes(),
            Err(e) => {
                error!(error = %e, "Failed to collect upstream response body");
                return Err(format!("Failed to read upstream body: {e}"));
            }
        };

        let len = collected.len();
        debug!(body_size = len, "Non-streaming response buffered");

        builder = builder.header("content-length", len.to_string());

        builder
            .body(Body::from(collected))
            .map_err(|e| format!("Failed to build buffered response: {e}"))
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hop_by_hop_list_is_sorted_lowercase() {
        // Ensure the list contains lowercase entries (as returned by HeaderName::as_str())
        for header in HOP_BY_HOP {
            assert_eq!(
                *header,
                header.to_lowercase(),
                "HOP_BY_HOP entry {header} is not lowercase"
            );
        }
    }
}
