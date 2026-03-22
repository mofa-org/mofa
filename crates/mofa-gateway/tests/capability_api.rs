use async_trait::async_trait;
use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use tower::ServiceExt;

use mofa_foundation::{
    CapabilityRequest, CapabilityResponse, GatewayCapability, GatewayCapabilityRegistry,
};
use mofa_gateway::{GatewayServer, ServerConfig};
use mofa_kernel::agent::types::error::GlobalResult;
use mofa_runtime::agent::registry::AgentRegistry;

struct EchoCapability;

#[async_trait]
impl GatewayCapability for EchoCapability {
    fn name(&self) -> &str {
        "web_search"
    }

    async fn invoke(&self, input: CapabilityRequest) -> GlobalResult<CapabilityResponse> {
        Ok(CapabilityResponse {
            output: format!("echo: {}", input.input),
            metadata: HashMap::from([(
                "trace_id".to_string(),
                Value::String(input.trace_id),
            )]),
            latency_ms: 3,
        })
    }
}

fn build_app() -> axum::Router {
    let registry = Arc::new(AgentRegistry::new());
    let capability_registry = Arc::new(GatewayCapabilityRegistry::new());
    capability_registry.register(Arc::new(EchoCapability));

    GatewayServer::new(ServerConfig::default(), registry)
        .with_capability_registry(capability_registry)
        .build_router()
}

async fn read_json(resp: axum::response::Response) -> Value {
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    serde_json::from_slice(&bytes).unwrap()
}

#[tokio::test]
async fn list_capabilities_returns_registered_names() {
    let app = build_app();

    let resp = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/capability/list")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
    let payload = read_json(resp).await;
    assert_eq!(
        payload.get("capabilities").and_then(|v| v.as_array()).unwrap(),
        &vec![Value::String("web_search".to_string())]
    );
}

#[tokio::test]
async fn invoke_capability_returns_capability_response() {
    let app = build_app();

    let body = serde_json::json!({
        "capability": "web_search",
        "input": "latest AI news",
        "params": {},
        "trace_id": "trace-capability"
    });

    let resp = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/capability/invoke")
                .header("content-type", "application/json")
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
    let payload = read_json(resp).await;
    assert_eq!(
        payload.get("output").and_then(|v| v.as_str()),
        Some("echo: latest AI news")
    );
    assert_eq!(
        payload.get("metadata").and_then(|v| v.get("trace_id")),
        Some(&Value::String("trace-capability".to_string()))
    );
}

#[tokio::test]
async fn invoke_unknown_capability_returns_404() {
    let app = build_app();

    let body = serde_json::json!({
        "capability": "missing",
        "input": "latest AI news",
        "params": {}
    });

    let resp = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/capability/invoke")
                .header("content-type", "application/json")
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    let payload = read_json(resp).await;
    assert!(payload.get("error").is_some());
}
