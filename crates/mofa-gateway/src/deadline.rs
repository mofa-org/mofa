//! Per-route deadline propagation and 504 enforcement.
//!
//! [`enforce_deadline`] wraps any async agent-call future with
//! `tokio::time::timeout`.  On expiry it cancels the agent future and returns
//! a [`GatewayResponse`] with status 504 and a structured JSON error body.
//!
//! # Effective timeout resolution
//!
//! The effective timeout for a request is resolved in this order:
//!
//! 1. `route.deadline.request_timeout_ms` — per-route override (if `Some`)
//! 2. `gateway_default_ms` — gateway-level default from `GatewayConfig`
//! 3. No timeout — the future is awaited without any time bound
//!
//! The resolved timeout (if any) is then used both to:
//! * stamp `RequestEnvelope::deadline` so downstream layers can inspect
//!   remaining time, and
//! * wrap the agent future with `tokio::time::timeout`.

use std::future::Future;
use std::time::Duration;

use mofa_kernel::gateway::envelope::{GatewayResponse, RequestEnvelope};
use mofa_kernel::gateway::route::GatewayRoute;

/// Resolve the effective `request_timeout_ms` for a route.
///
/// Returns `None` when neither the route nor the gateway default carries a
/// timeout (i.e. the request is unbounded).
pub fn resolve_timeout_ms(route: &GatewayRoute, gateway_default_ms: Option<u64>) -> Option<u64> {
    route
        .deadline
        .as_ref()
        .and_then(|d| d.request_timeout_ms)
        .or(gateway_default_ms)
}

/// Stamp `envelope.deadline` using the effective timeout for `route`.
///
/// If the resolved timeout is `None` the envelope is returned unchanged.
pub fn stamp_deadline(
    mut envelope: RequestEnvelope,
    route: &GatewayRoute,
    gateway_default_ms: Option<u64>,
) -> RequestEnvelope {
    if let Some(ms) = resolve_timeout_ms(route, gateway_default_ms) {
        envelope = envelope.with_timeout_ms(ms);
    }
    envelope
}

/// Enforce the deadline for a single agent dispatch.
///
/// Wraps `agent_fut` with `tokio::time::timeout` using the effective timeout
/// resolved from `route` and `gateway_default_ms`.  On expiry the agent
/// future is cancelled and a 504 [`GatewayResponse`] is returned.
///
/// If no timeout is configured the future is awaited without any time bound
/// and its result is returned directly.
///
/// # Type parameters
///
/// * `F` — an async future that returns a [`GatewayResponse`]
pub async fn enforce_deadline<F>(
    route: &GatewayRoute,
    gateway_default_ms: Option<u64>,
    agent_fut: F,
) -> GatewayResponse
where
    F: Future<Output = GatewayResponse>,
{
    match resolve_timeout_ms(route, gateway_default_ms) {
        Some(timeout_ms) => {
            match tokio::time::timeout(Duration::from_millis(timeout_ms), agent_fut).await {
                Ok(response) => response,
                Err(_elapsed) => GatewayResponse::deadline_exceeded(&route.id, timeout_ms),
            }
        }
        None => agent_fut.await,
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    use mofa_kernel::gateway::envelope::RequestEnvelope;
    use mofa_kernel::gateway::route::{GatewayRoute, HttpMethod, RouteDeadline};

    use super::*;

    fn route_with_timeout(ms: u64) -> GatewayRoute {
        GatewayRoute::new("test", "agent-test", "/v1/test", HttpMethod::Post)
            .with_deadline(RouteDeadline {
                request_timeout_ms: Some(ms),
                connect_timeout_ms: None,
                idle_timeout_ms: None,
            })
    }

    fn route_no_deadline() -> GatewayRoute {
        GatewayRoute::new("test", "agent-test", "/v1/test", HttpMethod::Post)
    }

    // ── resolve_timeout_ms ────────────────────────────────────────────────────

    #[test]
    fn per_route_timeout_takes_precedence_over_default() {
        let route = route_with_timeout(100);
        assert_eq!(resolve_timeout_ms(&route, Some(5_000)), Some(100));
    }

    #[test]
    fn falls_back_to_gateway_default_when_no_per_route_deadline() {
        let route = route_no_deadline();
        assert_eq!(resolve_timeout_ms(&route, Some(5_000)), Some(5_000));
    }

    #[test]
    fn returns_none_when_neither_route_nor_default_has_timeout() {
        let route = route_no_deadline();
        assert_eq!(resolve_timeout_ms(&route, None), None);
    }

    // ── stamp_deadline ────────────────────────────────────────────────────────

    #[test]
    fn stamp_deadline_sets_instant_within_tolerance() {
        let route = route_with_timeout(500);
        let env = RequestEnvelope::new("cid", "test", "/v1/test", "POST");
        let env = stamp_deadline(env, &route, None);

        let deadline = env.deadline.expect("deadline must be set");
        let expected = Instant::now() + Duration::from_millis(500);
        // Allow 50 ms of scheduling jitter.
        assert!(deadline <= expected + Duration::from_millis(50));
        assert!(!env.is_expired());
    }

    #[test]
    fn stamp_deadline_leaves_envelope_unchanged_when_no_timeout() {
        let route = route_no_deadline();
        let env = RequestEnvelope::new("cid", "test", "/v1/test", "POST");
        let env = stamp_deadline(env, &route, None);
        assert!(env.deadline.is_none());
    }

    // ── enforce_deadline ─────────────────────────────────────────────────────

    #[tokio::test]
    async fn agent_responding_within_deadline_returns_200() {
        let route = route_with_timeout(500);

        // Agent responds immediately — well within the 500 ms window.
        let response = enforce_deadline(&route, None, async {
            GatewayResponse::ok(serde_json::json!({ "reply": "hello" }))
        })
        .await;

        assert_eq!(response.status, 200);
        assert!(response.is_success());
    }

    #[tokio::test]
    async fn agent_exceeding_deadline_returns_504() {
        let route = route_with_timeout(50); // 50 ms

        // Agent sleeps longer than the deadline.
        let response = enforce_deadline(&route, None, async {
            tokio::time::sleep(Duration::from_millis(200)).await;
            GatewayResponse::ok(serde_json::json!({ "reply": "too late" }))
        })
        .await;

        assert_eq!(response.status, 504);
        assert_eq!(response.body["error"], "deadline_exceeded");
        assert_eq!(response.body["route_id"], "test");
        assert_eq!(response.body["timeout_ms"], 50);
    }

    #[tokio::test]
    async fn gateway_default_enforced_when_route_has_no_deadline() {
        let route = route_no_deadline();

        let response = enforce_deadline(&route, Some(50), async {
            tokio::time::sleep(Duration::from_millis(200)).await;
            GatewayResponse::ok(serde_json::json!({}))
        })
        .await;

        assert_eq!(response.status, 504);
        assert_eq!(response.body["timeout_ms"], 50);
    }

    #[tokio::test]
    async fn no_timeout_configured_awaits_future_normally() {
        let route = route_no_deadline();

        let response = enforce_deadline(&route, None, async {
            // A small sleep is fine here since there is no timeout to trigger.
            tokio::time::sleep(Duration::from_millis(10)).await;
            GatewayResponse::ok(serde_json::json!({ "ok": true }))
        })
        .await;

        assert_eq!(response.status, 200);
    }

    // ── RequestEnvelope helpers ───────────────────────────────────────────────

    #[test]
    fn envelope_remaining_returns_positive_duration_before_expiry() {
        let env = RequestEnvelope::new("c", "r", "/", "GET")
            .with_timeout_ms(1_000);
        let rem = env.remaining().expect("remaining must be Some");
        assert!(rem > Duration::ZERO);
        assert!(rem <= Duration::from_millis(1_000));
    }

    #[test]
    fn envelope_is_expired_false_for_future_deadline() {
        let env = RequestEnvelope::new("c", "r", "/", "GET")
            .with_timeout_ms(10_000);
        assert!(!env.is_expired());
    }
}
