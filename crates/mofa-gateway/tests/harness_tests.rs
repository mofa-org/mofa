//! Gateway integration tests using the in-process test harness.
//!
//! Each test exercises a specific gateway behaviour:
//!
//! | Test | What it verifies |
//! |------|-----------------|
//! | `registered_route_returns_backend_response` | Happy-path proxy dispatch |
//! | `unregistered_path_returns_404` | Missing-route handling |
//! | `missing_auth_returns_401` | Admin-API auth guard |
//! | `wrong_auth_key_returns_401` | Admin-API auth guard |
//! | `rate_limited_request_returns_429` | Per-route rate limiting + Retry-After |
//! | `slow_agent_exceeds_deadline_returns_504` | Deadline enforcement / 504 |
//! | `admin_api_list_and_deregister` | Admin list → deregister → 404 lifecycle |
//!
//! All tests are fully in-process: no LLM credentials, no real network access,
//! no environment variables required.

mod common;

use serde_json::json;

// ─────────────────────────────────────────────────────────────────────────────
// Happy path: registered route proxies to mock backend
// ─────────────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn registered_route_returns_backend_response() {
    let backend = common::MockAgentBackend::simple(json!({ "reply": "hello from agent" })).await;

    let harness = common::HarnessBuilder::default().build().await;

    harness
        .register_route(json!({
            "id": "chat",
            "path_pattern": "/v1/chat",
            "backend_url": backend.url(),
            "method": "GET"
        }))
        .await;

    let resp = harness
        .client
        .get(format!("{}/v1/chat", harness.url()))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["reply"], "hello from agent");
    assert_eq!(backend.requests_received(), 1);
}

// ─────────────────────────────────────────────────────────────────────────────
// Unregistered path returns 404
// ─────────────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn unregistered_path_returns_404() {
    let harness = common::HarnessBuilder::default().build().await;

    let resp = harness
        .client
        .get(format!("{}/does/not/exist", harness.url()))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 404);
}

// ─────────────────────────────────────────────────────────────────────────────
// Auth guard: missing key returns 401
// ─────────────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn missing_auth_returns_401() {
    let harness = common::HarnessBuilder::default().build().await;

    let resp = harness
        .client
        .get(format!("{}/admin/routes", harness.url()))
        // deliberately omit x-admin-key
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 401);
}

// ─────────────────────────────────────────────────────────────────────────────
// Auth guard: wrong key returns 401
// ─────────────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn wrong_auth_key_returns_401() {
    let harness = common::HarnessBuilder::default().build().await;

    let resp = harness
        .client
        .get(format!("{}/admin/routes", harness.url()))
        .header("x-admin-key", "definitely-wrong-key")
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 401);
}

// ─────────────────────────────────────────────────────────────────────────────
// Rate limiting: exceeding the limit returns 429 with Retry-After
// ─────────────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn rate_limited_request_returns_429_with_retry_after() {
    let backend = common::MockAgentBackend::simple(json!({ "ok": true })).await;

    // Allow only 2 requests per second.
    let harness = common::HarnessBuilder::default().build().await;
    harness
        .register_route(json!({
            "id": "limited",
            "path_pattern": "/v1/limited",
            "backend_url": backend.url(),
            "method": "GET",
            "max_requests_per_sec": 2
        }))
        .await;

    let url = format!("{}/v1/limited", harness.url());

    // First two requests should succeed.
    for _ in 0..2 {
        let resp = harness.client.get(&url).send().await.unwrap();
        assert_eq!(resp.status(), 200, "expected 200 for allowed request");
    }

    // Third request in the same window should be rate-limited.
    let resp = harness.client.get(&url).send().await.unwrap();
    assert_eq!(resp.status(), 429);
    assert!(
        resp.headers().contains_key("Retry-After"),
        "429 response must include Retry-After header"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// Deadline enforcement: slow agent returns 504
// ─────────────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn slow_agent_exceeds_deadline_returns_504() {
    // Backend sleeps 300 ms — well past our 50 ms deadline.
    let backend = common::MockAgentBackend::slow(300).await;

    let harness = common::HarnessBuilder::default().build().await;
    harness
        .register_route(json!({
            "id": "slow-route",
            "path_pattern": "/v1/slow",
            "backend_url": backend.url(),
            "method": "GET",
            "timeout_ms": 50
        }))
        .await;

    let resp = harness
        .client
        .get(format!("{}/v1/slow", harness.url()))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 504);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["error"], "deadline_exceeded");
    assert_eq!(body["route_id"], "slow-route");
    assert_eq!(body["timeout_ms"], 50);
}

// ─────────────────────────────────────────────────────────────────────────────
// Admin API: list → deregister → 404 lifecycle
// ─────────────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn admin_api_list_and_deregister_lifecycle() {
    let backend = common::MockAgentBackend::simple(json!({ "ok": true })).await;

    let harness = common::HarnessBuilder::default().build().await;

    // Register.
    let register_resp = harness
        .register_route(json!({
            "id": "lifecycle-route",
            "path_pattern": "/v1/lifecycle",
            "backend_url": backend.url(),
            "method": "GET"
        }))
        .await;
    assert_eq!(register_resp.status(), 201);

    // List — route appears.
    let list_resp = harness.list_routes().await;
    assert_eq!(list_resp.status(), 200);
    let routes: Vec<serde_json::Value> = list_resp.json().await.unwrap();
    assert_eq!(routes.len(), 1);
    assert_eq!(routes[0]["id"], "lifecycle-route");
    assert_eq!(routes[0]["enabled"], true);

    // Proxy request succeeds.
    let proxy_resp = harness
        .client
        .get(format!("{}/v1/lifecycle", harness.url()))
        .send()
        .await
        .unwrap();
    assert_eq!(proxy_resp.status(), 200);

    // Deregister.
    let dereg_resp = harness.deregister_route("lifecycle-route").await;
    assert_eq!(dereg_resp.status(), 200);

    // List — empty.
    let list_resp2 = harness.list_routes().await;
    let routes2: Vec<serde_json::Value> = list_resp2.json().await.unwrap();
    assert!(routes2.is_empty());

    // Proxy request now returns 404.
    let gone_resp = harness
        .client
        .get(format!("{}/v1/lifecycle", harness.url()))
        .send()
        .await
        .unwrap();
    assert_eq!(gone_resp.status(), 404);
}

// ─────────────────────────────────────────────────────────────────────────────
// Deregister missing route returns 404
// ─────────────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn deregister_nonexistent_route_returns_404() {
    let harness = common::HarnessBuilder::default().build().await;
    let resp = harness.deregister_route("ghost-route").await;
    assert_eq!(resp.status(), 404);
}

// ─────────────────────────────────────────────────────────────────────────────
// Gateway-level default timeout is applied when route has none
// ─────────────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn gateway_default_timeout_triggers_504() {
    // Backend sleeps 200 ms.
    let backend = common::MockAgentBackend::slow(200).await;

    // Harness has a 50 ms gateway-level default — no per-route timeout.
    let harness = common::HarnessBuilder::default()
        .default_timeout_ms(50)
        .build()
        .await;

    harness
        .register_route(json!({
            "id": "default-timeout-route",
            "path_pattern": "/v1/default-timeout",
            "backend_url": backend.url(),
            "method": "GET"
        }))
        .await;

    let resp = harness
        .client
        .get(format!("{}/v1/default-timeout", harness.url()))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 504);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["error"], "deadline_exceeded");
}

// ─────────────────────────────────────────────────────────────────────────────
// Error injection: seeded RNG produces deterministic error pattern
// ─────────────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn mock_backend_error_injection_is_deterministic() {
    use common::MockAgentConfig;

    // 100 % error rate — every request should return 500.
    let backend = common::MockAgentBackend::spawn(MockAgentConfig {
        error_rate: 1.0,
        rng_seed: 7,
        ..Default::default()
    })
    .await;

    let harness = common::HarnessBuilder::default().build().await;
    harness
        .register_route(json!({
            "id": "error-route",
            "path_pattern": "/v1/error",
            "backend_url": backend.url(),
            "method": "GET"
        }))
        .await;

    for _ in 0..3 {
        let resp = harness
            .client
            .get(format!("{}/v1/error", harness.url()))
            .send()
            .await
            .unwrap();
        assert_eq!(resp.status(), 500, "100% error rate must always return 500");
    }
}
