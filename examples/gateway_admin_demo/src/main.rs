//! Gateway Admin API — end-to-end demo
//!
//! Demonstrates the full route lifecycle against the MoFA admin REST API:
//!
//! 1. Start the admin server in-process on a random port
//! 2. `GET  /admin/health`              — confirm gateway is up
//! 3. `POST /admin/routes`              — register two routes at runtime
//! 4. `GET  /admin/routes`              — inspect the live route table
//! 5. `PATCH /admin/routes/:id`         — disable one route without deregistering
//! 6. `GET  /admin/routes`              — confirm enabled flag changed
//! 7. `DELETE /admin/routes/:id`        — deregister the disabled route
//! 8. `GET  /admin/routes`              — confirm only one route remains
//!
//! Run with:
//! ```sh
//! cargo run -p gateway_admin_demo
//! ```

use std::sync::Arc;

use axum::Router;
use axum::body::Body;
use axum::http::{Request, StatusCode};
use axum::response::Response;
use serde_json::{Value, json};
use tower::ServiceExt;
use tracing::info;

use mofa_gateway::admin::{AdminServer, AdminServerConfig, AdminState};

const ADMIN_KEY: &str = "demo-secret-key";

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_target(false)
        .with_level(true)
        .init();

    info!("MoFA Gateway Admin API Demo");
    info!("============================");

    // Build the admin server and get a handle to the shared state.
    let config = AdminServerConfig::new(ADMIN_KEY);
    let server = AdminServer::new(config);
    let state: Arc<AdminState> = Arc::clone(&server.state);
    let app: Router = server.build_router();

    // ── Step 1: Health check ──────────────────────────────────────────────────
    info!("\n[1] GET /admin/health");
    let resp: Response = app
        .clone()
        .oneshot(auth_get("/admin/health"))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = read_json(resp).await;
    info!("    uptime_secs     = {}", body["uptime_secs"]);
    info!("    routes_count    = {}", body["routes_count"]);
    info!("    hot_reload      = {}", body["hot_reload_active"]);
    info!("    version         = {}", body["version"]);

    // ── Step 2: Register route — chat ─────────────────────────────────────────
    info!("\n[2] POST /admin/routes  (chat agent)");
    let resp: Response = app
        .clone()
        .oneshot(auth_post(
            "/admin/routes",
            json!({
                "id": "chat",
                "path_pattern": "/v1/chat",
                "agent_id": "agent-chat",
                "method": "POST",
                "strategy": "weighted_round_robin"
            }),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);
    info!("    → {}", read_json(resp).await);

    // ── Step 3: Register route — summariser ───────────────────────────────────
    info!("\n[3] POST /admin/routes  (summariser agent)");
    let resp: Response = app
        .clone()
        .oneshot(auth_post(
            "/admin/routes",
            json!({
                "id": "summarise",
                "path_pattern": "/v1/summarise",
                "agent_id": "agent-summariser",
                "method": "POST",
                "strategy": "capability_match"
            }),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);
    info!("    → {}", read_json(resp).await);

    // ── Step 4: List routes ───────────────────────────────────────────────────
    info!("\n[4] GET /admin/routes");
    let resp: Response = app
        .clone()
        .oneshot(auth_get("/admin/routes"))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let routes = read_json(resp).await;
    info!("    {} routes registered:", routes.as_array().unwrap().len());
    for r in routes.as_array().unwrap() {
        info!(
            "      • {} → {}  [enabled={}  strategy={}]",
            r["path_pattern"], r["agent_id"], r["enabled"], r["strategy"]
        );
    }

    // ── Step 5: Simulate some traffic ─────────────────────────────────────────
    info!("\n[5] Simulating 5 requests on 'chat' route...");
    for _ in 0..5 {
        state.record_request("chat");
    }
    info!("    total_requests = {}", state.total_requests());

    // ── Step 6: Disable the summariser route ──────────────────────────────────
    info!("\n[6] PATCH /admin/routes/summarise  (disable)");
    let resp: Response = app
        .clone()
        .oneshot(auth_patch(
            "/admin/routes/summarise",
            json!({ "enabled": false }),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    info!("    → {}", read_json(resp).await);

    // ── Step 7: Confirm disabled ──────────────────────────────────────────────
    info!("\n[7] GET /admin/routes  (check enabled flags)");
    let resp: Response = app
        .clone()
        .oneshot(auth_get("/admin/routes"))
        .await
        .unwrap();
    let routes = read_json(resp).await;
    for r in routes.as_array().unwrap() {
        info!(
            "      • {}  enabled={}  requests={}",
            r["id"], r["enabled"], r["requests_served"]
        );
    }

    // ── Step 8: Deregister disabled route ────────────────────────────────────
    info!("\n[8] DELETE /admin/routes/summarise");
    let resp: Response = app
        .clone()
        .oneshot(auth_delete("/admin/routes/summarise"))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    info!("    → {}", read_json(resp).await);

    // ── Step 9: Confirm one route remains ────────────────────────────────────
    info!("\n[9] GET /admin/routes  (final state)");
    let resp: Response = app
        .clone()
        .oneshot(auth_get("/admin/routes"))
        .await
        .unwrap();
    let routes = read_json(resp).await;
    let arr = routes.as_array().unwrap();
    assert_eq!(arr.len(), 1, "expected exactly one route remaining");
    info!(
        "    {} route remaining: {}  requests={}",
        arr.len(),
        arr[0]["id"],
        arr[0]["requests_served"]
    );

    // ── Step 10: Auth guard ───────────────────────────────────────────────────
    info!("\n[10] Auth guard — wrong key should return 401");
    let req = Request::builder()
        .method("GET")
        .uri("/admin/routes")
        .header("x-admin-key", "wrong-key")
        .body(Body::empty())
        .unwrap();
    let resp: Response = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    info!("     → 401 UNAUTHORIZED (correct)");

    info!("\nDemo complete. All assertions passed.");
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn auth_get(uri: &str) -> Request<Body> {
    Request::builder()
        .method("GET")
        .uri(uri)
        .header("x-admin-key", ADMIN_KEY)
        .body(Body::empty())
        .unwrap()
}

fn auth_post(uri: &str, body: Value) -> Request<Body> {
    Request::builder()
        .method("POST")
        .uri(uri)
        .header("x-admin-key", ADMIN_KEY)
        .header("content-type", "application/json")
        .body(Body::from(body.to_string()))
        .unwrap()
}

fn auth_patch(uri: &str, body: Value) -> Request<Body> {
    Request::builder()
        .method("PATCH")
        .uri(uri)
        .header("x-admin-key", ADMIN_KEY)
        .header("content-type", "application/json")
        .body(Body::from(body.to_string()))
        .unwrap()
}

fn auth_delete(uri: &str) -> Request<Body> {
    Request::builder()
        .method("DELETE")
        .uri(uri)
        .header("x-admin-key", ADMIN_KEY)
        .body(Body::empty())
        .unwrap()
}

async fn read_json(resp: Response) -> Value {
    let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap();
    serde_json::from_slice(&bytes).unwrap_or(Value::Null)
}
