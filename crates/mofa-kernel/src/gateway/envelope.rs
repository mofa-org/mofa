//! Request envelope and gateway response types.
//!
//! [`RequestEnvelope`] is the canonical wrapper around an admitted request.
//! It carries the per-request deadline `Instant` so every layer downstream
//! (middleware, routing strategy, agent handler) can check how much time
//! remains without recomputing it from the original timeout.
//!
//! [`GatewayResponse`] is the typed response returned by the dispatch layer.
//! Using this struct (rather than ad-hoc strings) ensures that structured
//! error bodies like the 504 deadline-exceeded payload are machine-readable.

use std::collections::HashMap;
use std::time::Instant;

use serde::{Deserialize, Serialize};

// ─────────────────────────────────────────────────────────────────────────────
// RequestEnvelope
// ─────────────────────────────────────────────────────────────────────────────

/// A request after it has been admitted at the gateway boundary.
///
/// The `deadline` field holds the absolute `Instant` by which the response
/// must be sent.  It is computed once at admission time by adding the
/// effective `request_timeout_ms` (per-route if set, gateway default
/// otherwise) to `Instant::now()`.  A `None` value means no deadline is
/// configured for this request.
#[derive(Debug, Clone)]
pub struct RequestEnvelope {
    /// Per-request correlation ID for distributed tracing and log correlation.
    pub correlation_id: String,
    /// ID of the route that matched this request.
    pub route_id: String,
    /// Request path (e.g. `/v1/chat`).
    pub path: String,
    /// HTTP method string (e.g. `"POST"`).
    pub method: String,
    /// Lowercased request headers.
    pub headers: HashMap<String, String>,
    /// Raw request body bytes.
    pub body: Vec<u8>,
    /// Absolute deadline for the full request-response cycle.
    ///
    /// `None` means no timeout is enforced for this request.
    pub deadline: Option<Instant>,
}

impl RequestEnvelope {
    /// Create a new envelope without a deadline.
    pub fn new(
        correlation_id: impl Into<String>,
        route_id: impl Into<String>,
        path: impl Into<String>,
        method: impl Into<String>,
    ) -> Self {
        Self {
            correlation_id: correlation_id.into(),
            route_id: route_id.into(),
            path: path.into(),
            method: method.into(),
            headers: HashMap::new(),
            body: Vec::new(),
            deadline: None,
        }
    }

    /// Attach a deadline computed from a timeout duration in milliseconds.
    ///
    /// Calling this sets `deadline = Some(Instant::now() + timeout_ms)`.
    pub fn with_timeout_ms(mut self, timeout_ms: u64) -> Self {
        self.deadline = Some(
            Instant::now() + std::time::Duration::from_millis(timeout_ms),
        );
        self
    }

    /// Returns the remaining time before the deadline, or `None` if no
    /// deadline is set.  Returns `Some(Duration::ZERO)` if the deadline has
    /// already passed.
    pub fn remaining(&self) -> Option<std::time::Duration> {
        self.deadline.map(|d| {
            let now = Instant::now();
            if d > now { d - now } else { std::time::Duration::ZERO }
        })
    }

    /// Returns `true` if a deadline is set and it has already passed.
    pub fn is_expired(&self) -> bool {
        self.deadline.map(|d| Instant::now() >= d).unwrap_or(false)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// GatewayResponse
// ─────────────────────────────────────────────────────────────────────────────

/// A typed response produced by the gateway dispatch layer.
///
/// Using this struct ensures that error bodies (e.g. 504 deadline exceeded)
/// are machine-readable JSON rather than hand-rolled strings.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GatewayResponse {
    /// HTTP status code.
    pub status: u16,
    /// Response body serialised as a JSON value.
    pub body: serde_json::Value,
}

impl GatewayResponse {
    /// Create a successful (200) response with an arbitrary JSON body.
    pub fn ok(body: serde_json::Value) -> Self {
        Self { status: 200, body }
    }

    /// Create a 504 Gateway Timeout response for a deadline-exceeded event.
    ///
    /// The body format is:
    /// ```json
    /// { "error": "deadline_exceeded", "route_id": "...", "timeout_ms": ... }
    /// ```
    pub fn deadline_exceeded(route_id: &str, timeout_ms: u64) -> Self {
        Self {
            status: 504,
            body: serde_json::json!({
                "error": "deadline_exceeded",
                "route_id": route_id,
                "timeout_ms": timeout_ms,
            }),
        }
    }

    /// Returns `true` if the status code indicates success (2xx).
    pub fn is_success(&self) -> bool {
        self.status >= 200 && self.status < 300
    }
}
