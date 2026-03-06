//! Integration tests for the admin API.
//!
//! Spins up the admin router in-process using tower's `oneshot`, drives it
//! via axum test helpers, and verifies the full route lifecycle:
//! register → inspect → serve → toggle → deregister → 404.

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use serde_json::{Value, json};
    use tower::ServiceExt;

    use mofa_kernel::gateway::route::{GatewayRoute, HttpMethod};

    use crate::admin::handlers::admin_router;
    use crate::admin::state::AdminState;

    const ADMIN_KEY: &str = "test-key";

    fn build_state() -> Arc<AdminState> {
        Arc::new(AdminState::new(ADMIN_KEY))
    }

    fn build_app(state: Arc<AdminState>) -> axum::Router {
        admin_router().with_state(state)
    }

    async fn json_body(body: axum::body::Body) -> Value {
        let bytes = axum::body::to_bytes(body, usize::MAX).await.unwrap();
        serde_json::from_slice(&bytes).unwrap_or(Value::Null)
    }

    // ── Auth guard ────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn missing_key_returns_401() {
        let app = build_app(build_state());
        let req = Request::builder()
            .method("GET")
            .uri("/admin/routes")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn wrong_key_returns_401() {
        let app = build_app(build_state());
        let req = Request::builder()
            .method("GET")
            .uri("/admin/routes")
            .header("x-admin-key", "wrong")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    }

    // ── GET /admin/routes ─────────────────────────────────────────────────────

    #[tokio::test]
    async fn list_routes_empty() {
        let app = build_app(build_state());
        let req = Request::builder()
            .method("GET")
            .uri("/admin/routes")
            .header("x-admin-key", ADMIN_KEY)
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let body = json_body(resp.into_body()).await;
        assert_eq!(body, json!([]));
    }

    // ── POST /admin/routes ────────────────────────────────────────────────────

    #[tokio::test]
    async fn register_route_success() {
        let state = build_state();
        let app = build_app(Arc::clone(&state));
        let req = Request::builder()
            .method("POST")
            .uri("/admin/routes")
            .header("x-admin-key", ADMIN_KEY)
            .header("content-type", "application/json")
            .body(Body::from(
                json!({
                    "id": "chat",
                    "path_pattern": "/v1/chat",
                    "agent_id": "agent-chat",
                    "method": "POST"
                })
                .to_string(),
            ))
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::CREATED);
        assert_eq!(state.routes_count(), 1);
    }

    #[tokio::test]
    async fn register_route_duplicate_returns_409() {
        let state = build_state();
        let app = build_app(Arc::clone(&state));

        let body = json!({
            "id": "chat",
            "path_pattern": "/v1/chat",
            "agent_id": "agent-chat",
            "method": "POST"
        })
        .to_string();

        let req = Request::builder()
            .method("POST")
            .uri("/admin/routes")
            .header("x-admin-key", ADMIN_KEY)
            .header("content-type", "application/json")
            .body(Body::from(body.clone()))
            .unwrap();
        app.clone().oneshot(req).await.unwrap();

        let req2 = Request::builder()
            .method("POST")
            .uri("/admin/routes")
            .header("x-admin-key", ADMIN_KEY)
            .header("content-type", "application/json")
            .body(Body::from(body))
            .unwrap();
        let resp = app.oneshot(req2).await.unwrap();
        assert_eq!(resp.status(), StatusCode::CONFLICT);
    }

    #[tokio::test]
    async fn register_route_invalid_path_returns_422() {
        let app = build_app(build_state());
        let req = Request::builder()
            .method("POST")
            .uri("/admin/routes")
            .header("x-admin-key", ADMIN_KEY)
            .header("content-type", "application/json")
            .body(Body::from(
                json!({
                    "id": "bad",
                    "path_pattern": "no-leading-slash",
                    "agent_id": "agent-a",
                    "method": "GET"
                })
                .to_string(),
            ))
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::UNPROCESSABLE_ENTITY);
    }

    // ── PATCH /admin/routes/:id ───────────────────────────────────────────────

    #[tokio::test]
    async fn toggle_route_disable_enable() {
        let state = build_state();
        state.register_route(GatewayRoute::new(
            "chat",
            "agent-chat",
            "/v1/chat",
            HttpMethod::Post,
        ));

        // Disable.
        let app = build_app(Arc::clone(&state));
        let req = Request::builder()
            .method("PATCH")
            .uri("/admin/routes/chat")
            .header("x-admin-key", ADMIN_KEY)
            .header("content-type", "application/json")
            .body(Body::from(json!({ "enabled": false }).to_string()))
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        // Verify disabled.
        let routes = state.list_routes();
        assert_eq!(routes.len(), 1);
        assert!(!routes[0].0.enabled);

        // Re-enable.
        let app = build_app(Arc::clone(&state));
        let req = Request::builder()
            .method("PATCH")
            .uri("/admin/routes/chat")
            .header("x-admin-key", ADMIN_KEY)
            .header("content-type", "application/json")
            .body(Body::from(json!({ "enabled": true }).to_string()))
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let routes = state.list_routes();
        assert!(routes[0].0.enabled);
    }

    #[tokio::test]
    async fn toggle_missing_route_returns_404() {
        let app = build_app(build_state());
        let req = Request::builder()
            .method("PATCH")
            .uri("/admin/routes/nonexistent")
            .header("x-admin-key", ADMIN_KEY)
            .header("content-type", "application/json")
            .body(Body::from(json!({ "enabled": false }).to_string()))
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }

    // ── DELETE /admin/routes/:id ──────────────────────────────────────────────

    #[tokio::test]
    async fn deregister_route_success() {
        let state = build_state();
        state.register_route(GatewayRoute::new(
            "chat",
            "agent-chat",
            "/v1/chat",
            HttpMethod::Post,
        ));

        let app = build_app(Arc::clone(&state));
        let req = Request::builder()
            .method("DELETE")
            .uri("/admin/routes/chat")
            .header("x-admin-key", ADMIN_KEY)
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        assert_eq!(state.routes_count(), 0);
    }

    #[tokio::test]
    async fn deregister_missing_route_returns_404() {
        let app = build_app(build_state());
        let req = Request::builder()
            .method("DELETE")
            .uri("/admin/routes/nonexistent")
            .header("x-admin-key", ADMIN_KEY)
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }

    // ── GET /admin/health ─────────────────────────────────────────────────────

    #[tokio::test]
    async fn health_returns_summary() {
        let app = build_app(build_state());
        let req = Request::builder()
            .method("GET")
            .uri("/admin/health")
            .header("x-admin-key", ADMIN_KEY)
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let body = json_body(resp.into_body()).await;
        assert!(body.get("uptime_secs").is_some());
        assert!(body.get("total_requests").is_some());
        assert!(body.get("routes_count").is_some());
        assert!(body.get("hot_reload_active").is_some());
        assert!(body.get("version").is_some());
    }

    // ── Full lifecycle ────────────────────────────────────────────────────────

    #[tokio::test]
    async fn route_lifecycle_register_inspect_toggle_deregister() {
        let state = build_state();

        // 1. Register.
        let req = Request::builder()
            .method("POST")
            .uri("/admin/routes")
            .header("x-admin-key", ADMIN_KEY)
            .header("content-type", "application/json")
            .body(Body::from(
                json!({
                    "id": "lifecycle",
                    "path_pattern": "/v1/lifecycle",
                    "agent_id": "agent-lc",
                    "method": "GET"
                })
                .to_string(),
            ))
            .unwrap();
        let app = build_app(Arc::clone(&state));
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::CREATED);

        // 2. Inspect — route appears in list, enabled by default.
        let req = Request::builder()
            .method("GET")
            .uri("/admin/routes")
            .header("x-admin-key", ADMIN_KEY)
            .body(Body::empty())
            .unwrap();
        let app = build_app(Arc::clone(&state));
        let resp = app.oneshot(req).await.unwrap();
        let body = json_body(resp.into_body()).await;
        let routes = body.as_array().unwrap();
        assert_eq!(routes.len(), 1);
        assert_eq!(routes[0]["id"], "lifecycle");
        assert_eq!(routes[0]["enabled"], true);

        // 3. Record a request (simulate serving).
        state.record_request("lifecycle");
        assert_eq!(state.total_requests(), 1);

        // 4. Disable route.
        let req = Request::builder()
            .method("PATCH")
            .uri("/admin/routes/lifecycle")
            .header("x-admin-key", ADMIN_KEY)
            .header("content-type", "application/json")
            .body(Body::from(json!({ "enabled": false }).to_string()))
            .unwrap();
        let app = build_app(Arc::clone(&state));
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        // 5. Deregister.
        let req = Request::builder()
            .method("DELETE")
            .uri("/admin/routes/lifecycle")
            .header("x-admin-key", ADMIN_KEY)
            .body(Body::empty())
            .unwrap();
        let app = build_app(Arc::clone(&state));
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        // 6. Confirm gone — list is empty again.
        let req = Request::builder()
            .method("GET")
            .uri("/admin/routes")
            .header("x-admin-key", ADMIN_KEY)
            .body(Body::empty())
            .unwrap();
        let app = build_app(Arc::clone(&state));
        let resp = app.oneshot(req).await.unwrap();
        let body = json_body(resp.into_body()).await;
        assert_eq!(body, json!([]));
    }
}
