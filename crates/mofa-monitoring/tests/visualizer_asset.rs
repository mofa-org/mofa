use axum::body::{Body, to_bytes};
use axum::http::{Request, StatusCode};
use mofa_monitoring::{DashboardConfig, DashboardServer};
use tower::ServiceExt;

#[tokio::test]
async fn visualizer_page_is_served_from_assets() {
    let mut server = DashboardServer::new(DashboardConfig::new());
    let app = server.build_router();

    let response = app
        .oneshot(
            Request::builder()
                .uri("/assets/visualizer")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .expect("request success");
    assert_eq!(response.status(), StatusCode::OK);
    let content_type = response
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default()
        .to_string();
    assert!(content_type.starts_with("text/html"));

    let body = to_bytes(response.into_body(), 5 * 1024 * 1024)
        .await
        .expect("read body");
    let body = String::from_utf8(body.to_vec()).expect("utf8");
    assert!(body.contains("MoFA Workflow Visualizer"));
}
