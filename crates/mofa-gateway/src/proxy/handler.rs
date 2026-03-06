//! HTTP proxy handler implementation.

use super::config::ProxyBackend;
use crate::error::{GatewayError, GatewayResult};
use axum::body::Body;
use axum::http::{HeaderMap, Method, Request, Response, StatusCode, Uri};
use bytes::Bytes;
use hyper_util::client::legacy::connect::HttpConnector;
use hyper_util::client::legacy::Client;
use hyper_util::rt::TokioExecutor;
use std::sync::Arc;
use std::time::Duration;
use tracing::{debug, error, warn};

type HttpClient = Client<HttpConnector, Body>;

/// HTTP proxy handler for forwarding requests to backend services.
pub struct ProxyHandler {
    backend: ProxyBackend,
    client: HttpClient,
}

impl ProxyHandler {
    /// Create a new proxy handler for the given backend.
    pub fn new(backend: ProxyBackend) -> Self {
        let client = Client::builder(TokioExecutor::new())
            .build_http::<Body>();

        Self { backend, client }
    }

    /// Forward an HTTP request to the backend service.
    ///
    /// This method:
    /// 1. Constructs the target URL by combining backend base_url with the request path
    /// 2. Copies relevant headers from the incoming request
    /// 3. Forwards the request body
    /// 4. Returns the backend's response
    pub async fn forward(
        &self,
        request: Request<Body>,
        path: &str,
    ) -> GatewayResult<Response<Body>> {
        // Extract parts from request
        let (parts, body) = request.into_parts();

        // Build target URL
        let target_url = self.build_target_url(path)?;
        let target_uri: Uri = target_url
            .parse()
            .map_err(|e| GatewayError::Network(format!("Invalid target URL: {}", e)))?;

        // Create new request with target URI
        let mut proxy_request = Request::builder()
            .method(parts.method.clone())
            .uri(target_uri)
            .body(body)
            .map_err(|e| GatewayError::Network(format!("Failed to build proxy request: {}", e)))?;

        // Copy headers (excluding hop-by-hop headers)
        self.copy_headers(&parts.headers, proxy_request.headers_mut());

        // Create tracing span for proxy request
        let span = tracing::info_span!(
            "proxy_forward",
            backend = %self.backend.name,
            method = %proxy_request.method(),
            url = %proxy_request.uri(),
            status = tracing::field::Empty,
        );
        let _enter = span.enter();

        debug!(
            backend = %self.backend.name,
            method = %proxy_request.method(),
            url = %proxy_request.uri(),
            "Forwarding request to backend"
        );

        // Forward request with timeout
        let response = tokio::time::timeout(self.backend.timeout, async {
            self.client
                .request(proxy_request)
                .await
                .map_err(|e| GatewayError::Network(format!("Request failed: {}", e)))
        })
        .await
        .map_err(|_| GatewayError::Network("Request timeout".to_string()))??;

        debug!(
            backend = %self.backend.name,
            status = %response.status(),
            "Received response from backend"
        );
        
        // Record response status in span
        tracing::Span::current().record("status", response.status().as_u16());

        // Convert hyper response to axum response
        // The response is already a Response<Body> from hyper client, but we need to convert
        // the body from hyper::body::Incoming to axum::body::Body
        let (parts, body) = response.into_parts();
        
        // Convert hyper Incoming body to bytes using http_body_util
        use http_body_util::BodyExt;
        let body_bytes = match body.collect().await {
            Ok(collected) => collected.to_bytes(),
            Err(e) => {
                error!(backend = %self.backend.name, error = %e, "Failed to collect response body");
                return Err(GatewayError::Network(format!("Failed to read response body: {}", e)));
            }
        };
        
        let body_size = body_bytes.len();
        debug!(
            backend = %self.backend.name,
            body_size = body_size,
            "Converted response body to bytes"
        );
        
        // Build axum response with headers
        let mut response_builder = Response::builder().status(parts.status);
        
        // Copy headers, excluding hop-by-hop headers and normalizing content-length/transfer-encoding
        for (key, value) in parts.headers.iter() {
            let skip = matches!(
                key.as_str(),
                "connection" | "keep-alive" | "proxy-authenticate" | "proxy-authorization"
                    | "te" | "trailers" | "transfer-encoding" | "upgrade"
            );
            
            if !skip {
                response_builder = response_builder.header(key.clone(), value.clone());
            }
        }
        
        // Set content-length for fixed body (since we buffered it)
        response_builder = response_builder.header("content-length", body_size.to_string());
        
        // Set body - body_bytes is moved here
        let axum_response = response_builder
            .body(Body::from(body_bytes))
            .map_err(|e| GatewayError::Network(format!("Failed to build response: {}", e)))?;

        debug!(
            backend = %self.backend.name,
            status = %axum_response.status(),
            body_size = body_size,
            "Built axum response"
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
        Self::new(self.backend.clone())
    }
}
