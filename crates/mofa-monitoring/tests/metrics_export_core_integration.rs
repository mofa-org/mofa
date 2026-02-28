use axum::body::{Body, to_bytes};
use axum::http::{Request, StatusCode};
use mofa_monitoring::{AgentMetrics, DashboardConfig, DashboardServer};
use tower::ServiceExt;

#[tokio::test]
async fn metrics_route_returns_prometheus_payload() {
    let mut server = DashboardServer::new(DashboardConfig::new());

    server
        .collector()
        .update_agent(AgentMetrics {
            agent_id: "agent-alpha".to_string(),
            tasks_completed: 42,
            ..Default::default()
        })
        .await;

    let _ = server.collector().collect().await;
    let app = server.build_router();

    let response = app
        .oneshot(
            Request::builder()
                .uri("/metrics")
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("response");

    assert_eq!(response.status(), StatusCode::OK);

    let content_type = response
        .headers()
        .get("content-type")
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default();
    assert!(content_type.starts_with("text/plain"));

    let body = to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("read body");
    let body_str = String::from_utf8(body.to_vec()).expect("utf8");

    assert!(body_str.contains("# HELP mofa_agent_tasks_total"));
    assert!(body_str.contains("mofa_agent_tasks_total{agent_id=\"agent-alpha\"} 42"));
}
