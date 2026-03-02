//! Core data types for the gateway kernel contract.
//!
//! These types are shared across all gateway traits
//! ([`GatewayRouter`](super::router::GatewayRouter),
//! [`GatewayFilter`](super::filter::GatewayFilter),
//! [`CapabilityRegistry`](super::capability::CapabilityRegistry))
//! and carry no runtime dependencies beyond `serde` and `std`.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ─────────────────────────────────────────────────────────────────────────────
// HTTP primitives
// ─────────────────────────────────────────────────────────────────────────────

/// HTTP method, covering the standard verbs used in REST and proxy scenarios.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[non_exhaustive]
pub enum HttpMethod {
    Get,
    Post,
    Put,
    Patch,
    Delete,
    Head,
    Options,
}

impl HttpMethod {
    /// Case-insensitive parse from a string slice.
    pub fn from_str_ci(s: &str) -> Option<Self> {
        match s.to_uppercase().as_str() {
            "GET" => Some(HttpMethod::Get),
            "POST" => Some(HttpMethod::Post),
            "PUT" => Some(HttpMethod::Put),
            "PATCH" => Some(HttpMethod::Patch),
            "DELETE" => Some(HttpMethod::Delete),
            "HEAD" => Some(HttpMethod::Head),
            "OPTIONS" => Some(HttpMethod::Options),
            _ => None,
        }
    }

    /// Return the standard uppercase string representation.
    pub fn as_str(&self) -> &'static str {
        match self {
            HttpMethod::Get => "GET",
            HttpMethod::Post => "POST",
            HttpMethod::Put => "PUT",
            HttpMethod::Patch => "PATCH",
            HttpMethod::Delete => "DELETE",
            HttpMethod::Head => "HEAD",
            HttpMethod::Options => "OPTIONS",
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Request / Response
// ─────────────────────────────────────────────────────────────────────────────

/// An inbound request flowing through the gateway.
///
/// All fields use owned, allocation-friendly types so the struct can be sent
/// across async task boundaries without lifetime complications.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GatewayRequest {
    /// Unique identifier for correlating this request across logs and traces.
    pub id: String,
    /// Request path, e.g. `/v1/chat/completions`.
    pub path: String,
    /// HTTP method.
    pub method: HttpMethod,
    /// HTTP headers (header names are lowercased).
    pub headers: HashMap<String, String>,
    /// Raw body bytes.
    pub body: Vec<u8>,
    /// Arbitrary metadata attached by filters during processing.
    pub metadata: HashMap<String, serde_json::Value>,
}

impl GatewayRequest {
    /// Construct a minimal request with the given id, path, and method.
    pub fn new(
        id: impl Into<String>,
        path: impl Into<String>,
        method: HttpMethod,
    ) -> Self {
        Self {
            id: id.into(),
            path: path.into(),
            method,
            headers: HashMap::new(),
            body: Vec::new(),
            metadata: HashMap::new(),
        }
    }

    /// Builder helper: attach a header.
    pub fn with_header(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.headers.insert(key.into().to_lowercase(), value.into());
        self
    }

    /// Builder helper: set the body.
    pub fn with_body(mut self, body: Vec<u8>) -> Self {
        self.body = body;
        self
    }
}

/// An outbound response produced by a backend and returned through the gateway.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GatewayResponse {
    /// HTTP status code (100–599).
    pub status: u16,
    /// Response headers.
    pub headers: HashMap<String, String>,
    /// Raw body bytes.
    pub body: Vec<u8>,
    /// Id of the backend that generated this response.
    pub backend_id: String,
    /// Round-trip latency in milliseconds (gateway → backend → gateway).
    pub latency_ms: u64,
}

impl GatewayResponse {
    /// Construct a minimal response.
    pub fn new(status: u16, backend_id: impl Into<String>) -> Self {
        Self {
            status,
            headers: HashMap::new(),
            body: Vec::new(),
            backend_id: backend_id.into(),
            latency_ms: 0,
        }
    }

    /// Builder helper: attach a header.
    pub fn with_header(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.headers.insert(key.into().to_lowercase(), value.into());
        self
    }

    /// Builder helper: set the body.
    pub fn with_body(mut self, body: Vec<u8>) -> Self {
        self.body = body;
        self
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Route match
// ─────────────────────────────────────────────────────────────────────────────

/// The result of a successful route lookup.
///
/// Carries the matched route's configuration plus any path parameters
/// extracted during matching (e.g. `model_id → "gpt-4"` for the pattern
/// `/v1/models/{model_id}`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouteMatch {
    /// Id of the matched route.
    pub route_id: String,
    /// Id of the backend this route targets.
    pub backend_id: String,
    /// Path parameters extracted from the URL template.
    pub path_params: HashMap<String, String>,
    /// Configured timeout for this route in milliseconds.
    pub timeout_ms: u64,
}

// ─────────────────────────────────────────────────────────────────────────────
// Request context
// ─────────────────────────────────────────────────────────────────────────────

/// Mutable context that flows through the filter chain for a single request.
///
/// Filters read from and write to this context, enabling downstream filters
/// to access decisions made by upstream filters (e.g. the auth principal set
/// by the authentication filter can be read by the audit logger).
#[derive(Debug, Clone)]
pub struct GatewayContext {
    /// The inbound request.
    pub request: GatewayRequest,
    /// Populated after routing; `None` if routing has not yet occurred.
    pub route_match: Option<RouteMatch>,
    /// Identity principal resolved by the auth filter; `None` if unauthenticated.
    pub auth_principal: Option<String>,
    /// Free-form attributes written and read by filters.
    pub attributes: HashMap<String, serde_json::Value>,
}

impl GatewayContext {
    /// Create a fresh context from an inbound request.
    pub fn new(request: GatewayRequest) -> Self {
        Self {
            request,
            route_match: None,
            auth_principal: None,
            attributes: HashMap::new(),
        }
    }

    /// Convenience: read a typed attribute, returning `None` if absent or
    /// if deserialization fails.
    pub fn get_attr<T: serde::de::DeserializeOwned>(&self, key: &str) -> Option<T> {
        self.attributes
            .get(key)
            .and_then(|v| serde_json::from_value(v.clone()).ok())
    }

    /// Convenience: write a serializable attribute.
    pub fn set_attr<T: serde::Serialize>(&mut self, key: impl Into<String>, val: &T) {
        if let Ok(v) = serde_json::to_value(val) {
            self.attributes.insert(key.into(), v);
        }
    }
}
