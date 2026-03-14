//! HTTP proxy handler implementation.

use super::config::ProxyBackend;
use crate::error::{GatewayError, GatewayResult};
use axum::body::Body;
use axum::http::{HeaderMap, Method, Request, Response, Uri};
use http_body_util::BodyExt;
use hyper_util::client::legacy::connect::HttpConnector;
use hyper_util::client::legacy::Client;
use hyper_util::rt::TokioExecutor;
use std::sync::Arc;
use std::time::Duration;
use tracing::{debug, warn, Instrument};

type HttpClient = Client<HttpConnector, Body>;

/// HTTP proxy handler for forwarding requests to backend services.
pub struct ProxyHandler {
    backend: ProxyBackend,
    /// Shared HTTP client — Arc so cloning the handler reuses the connection pool.
    client: Arc<HttpClient>,
}

impl ProxyHandler {
    /// Create a new proxy handler for the given backend.
    pub fn new(backend: ProxyBackend) -> Self {
        let client = Arc::new(Client::builder(TokioExecutor::new()).build_http::<Body>());

        Self { backend, client }
    }

    /// Forward an HTTP request to the backend service.
    ///
    /// The response body is streamed through without buffering, so SSE / chunked
    /// streaming responses work correctly end-to-end.
    pub async fn forward(
        &self,
        request: Request<Body>,
        path: &str,
    ) -> GatewayResult<Response<Body>> {
        let (parts, body) = request.into_parts();

        let target_url = self.build_target_url(path)?;
        let target_uri: Uri = target_url
            .parse()
            .map_err(|e| GatewayError::Network(format!("Invalid target URL: {}", e)))?;

        let mut proxy_request = Request::builder()
            .method(parts.method.clone())
            .uri(target_uri)
            .body(body)
            .map_err(|e| GatewayError::Network(format!("Failed to build proxy request: {}", e)))?;

        self.copy_headers(&parts.headers, proxy_request.headers_mut());

        debug!(
            backend = %self.backend.name,
            method = %proxy_request.method(),
            url = %proxy_request.uri(),
            "Forwarding request to backend"
        );

        // Create the span before consuming proxy_request; use .instrument() so the
        // span stays active across the await point (fixes the dropped _enter bug).
        let span = tracing::info_span!(
            "proxy_forward",
            backend = %self.backend.name,
            status = tracing::field::Empty,
        );

        let response = async {
            tokio::time::timeout(self.backend.timeout, self.client.request(proxy_request))
                .await
                .map_err(|_| GatewayError::Network("Request timeout".to_string()))?
                .map_err(|e| GatewayError::Network(format!("Request failed: {}", e)))
        }
        .instrument(span.clone())
        .await?;

        span.record("status", response.status().as_u16());

        debug!(
            backend = %self.backend.name,
            status = %response.status(),
            "Received response from backend"
        );

        let (resp_parts, resp_body) = response.into_parts();

        let mut response_builder = Response::builder().status(resp_parts.status);

        // Copy headers, excluding hop-by-hop headers.
        // The backend's content-length is preserved as-is; we never add a second one.
        for (key, value) in resp_parts.headers.iter() {
            let skip = matches!(
                key.as_str(),
                "connection"
                    | "keep-alive"
                    | "proxy-authenticate"
                    | "proxy-authorization"
                    | "te"
                    | "trailers"
                    | "transfer-encoding"
                    | "upgrade"
            );
            if !skip {
                response_builder = response_builder.header(key.clone(), value.clone());
            }
        }

        // Stream the body through without buffering — supports both regular JSON
        // responses and SSE / chunked streaming (stream: true).
        let axum_response = response_builder
            .body(Body::from_stream(resp_body.into_data_stream()))
            .map_err(|e| GatewayError::Network(format!("Failed to build response: {}", e)))?;

        debug!(
            backend = %self.backend.name,
            status = %axum_response.status(),
            "Proxied response to client"
        );

        Ok(axum_response)
    }

    /// Perform a health check on the backend.
    pub async fn health_check(&self) -> GatewayResult<bool> {
        let health_url = if let Some(ref endpoint) = self.backend.health_check_endpoint {
            format!("{}{}", self.backend.base_url, endpoint)
        } else {
            return Ok(false);
        };

        let uri: Uri = health_url
            .parse()
            .map_err(|e| GatewayError::Network(format!("Invalid health check URL: {}", e)))?;

        let request = Request::builder()
            .method(Method::GET)
            .uri(uri)
            .body(Body::empty())
            .map_err(|e| GatewayError::Network(format!("Failed to build health check request: {}", e)))?;

        match tokio::time::timeout(Duration::from_secs(5), self.client.request(request)).await {
            Ok(Ok(response)) => Ok(response.status().is_success()),
            Ok(Err(e)) => {
                warn!(backend = %self.backend.name, error = %e, "Health check failed");
                Ok(false)
            }
            Err(_) => {
                warn!(backend = %self.backend.name, "Health check timeout");
                Ok(false)
            }
        }
    }

    /// Build the target URL by combining base_url with the request path.
    fn build_target_url(&self, path: &str) -> GatewayResult<String> {
        let path = path.trim_start_matches('/');
        Ok(format!("{}/{}", self.backend.base_url.trim_end_matches('/'), path))
    }

    /// Copy headers from source to destination, excluding hop-by-hop headers.
    fn copy_headers(&self, source: &HeaderMap, dest: &mut HeaderMap) {
        for (key, value) in source.iter() {
            // Skip hop-by-hop headers and host header
            // HeaderName::as_str() is already normalized to lowercase for standard headers
            let skip = matches!(
                key.as_str(),
                "connection" | "keep-alive" | "proxy-authenticate" | "proxy-authorization"
                    | "te" | "trailers" | "transfer-encoding" | "upgrade" | "host"
            );

            if !skip {
                dest.insert(key.clone(), value.clone());
            }
        }
    }

    /// Get the backend name.
    pub fn backend_name(&self) -> &str {
        &self.backend.name
    }

    /// Get the backend base URL.
    pub fn backend_url(&self) -> &str {
        &self.backend.base_url
    }
}

impl Clone for ProxyHandler {
    fn clone(&self) -> Self {
        // Reuse the shared connection pool instead of creating a new HTTP client.
        Self {
            backend: self.backend.clone(),
            client: Arc::clone(&self.client),
        }
    }
}
