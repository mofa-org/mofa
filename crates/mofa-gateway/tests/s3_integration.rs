//! Integration tests for the S3 file-storage endpoints.
//!
//! Uses an in-memory `MockObjectStore` injected via `GatewayServer::with_s3`
//! so no real AWS / MinIO instance is required.
//!
//! Run:
//! ```bash
//! cargo test -p mofa-gateway --test s3_integration
//! ```

use async_trait::async_trait;
use axum::body::Body;
use axum::http::{Request, StatusCode};
use mofa_gateway::server::{GatewayServer, ServerConfig};
use mofa_kernel::ObjectStore;
use mofa_kernel::agent::error::{AgentError, AgentResult};
use mofa_runtime::agent::registry::AgentRegistry;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tower::ServiceExt;

// ─────────────────────────────────────────────────────────────────────────────
// In-memory mock
// ─────────────────────────────────────────────────────────────────────────────

struct MockObjectStore {
    data: Arc<RwLock<HashMap<String, Vec<u8>>>>,
}

impl MockObjectStore {
    fn new() -> Arc<Self> {
        Arc::new(Self {
            data: Arc::new(RwLock::new(HashMap::new())),
        })
    }
}

#[async_trait]
impl ObjectStore for MockObjectStore {
    async fn put(&self, key: &str, data: Vec<u8>) -> AgentResult<()> {
        self.data.write().await.insert(key.to_string(), data);
        Ok(())
    }

    async fn get(&self, key: &str) -> AgentResult<Option<Vec<u8>>> {
        Ok(self.data.read().await.get(key).cloned())
    }

    async fn delete(&self, key: &str) -> AgentResult<bool> {
        Ok(self.data.write().await.remove(key).is_some())
    }

    async fn list_keys(&self, prefix: &str) -> AgentResult<Vec<String>> {
        let guard = self.data.read().await;
        let mut keys: Vec<String> = guard
            .keys()
            .filter(|k| k.starts_with(prefix))
            .cloned()
            .collect();
        keys.sort();
        Ok(keys)
    }

    async fn presigned_get_url(&self, key: &str, expires: u64) -> AgentResult<String> {
        Ok(format!(
            "https://mock.example.com/{}?expires={}",
            key, expires
        ))
    }

    async fn presigned_put_url(
        &self,
        key: &str,
        expires: u64,
        content_type: Option<&str>,
    ) -> AgentResult<String> {
        let ct = content_type.unwrap_or("*/*");
        Ok(format!(
            "https://mock.example.com/{}?method=PUT&expires={}&ct={}",
            key, expires, ct
        ))
    }

    async fn get_metadata(
        &self,
        key: &str,
    ) -> mofa_kernel::agent::error::AgentResult<Option<mofa_kernel::storage::ObjectMetadata>> {
        let guard = self.data.read().await;
        match guard.get(key) {
            Some(data) => Ok(Some(mofa_kernel::storage::ObjectMetadata {
                size: data.len() as u64,
                content_type: None,
                last_modified: Some("2025-01-01T00:00:00+00:00".to_string()),
            })),
            None => Ok(None),
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Helpers
// ─────────────────────────────────────────────────────────────────────────────

fn build_server_no_s3() -> GatewayServer {
    let registry = Arc::new(AgentRegistry::new());
    let config = ServerConfig::new()
        .with_host("127.0.0.1")
        .with_port(0)
        .with_cors(false);
    GatewayServer::new(config, registry)
}

fn build_server_with_s3() -> GatewayServer {
    let registry = Arc::new(AgentRegistry::new());
    let config = ServerConfig::new()
        .with_host("127.0.0.1")
        .with_port(0)
        .with_cors(false)
        .with_rate_limit(10_000, Duration::from_secs(60));
    GatewayServer::new(config, registry).with_s3(MockObjectStore::new() as Arc<dyn ObjectStore>)
}

fn build_server_with_size_limit(max_bytes: u64) -> GatewayServer {
    let registry = Arc::new(AgentRegistry::new());
    let config = ServerConfig::new()
        .with_host("127.0.0.1")
        .with_port(0)
        .with_cors(false)
        .with_rate_limit(10_000, Duration::from_secs(60))
        .with_max_upload_size(max_bytes);
    GatewayServer::new(config, registry).with_s3(MockObjectStore::new() as Arc<dyn ObjectStore>)
}

async fn body_json(resp: axum::response::Response) -> Value {
    let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap();
    serde_json::from_slice(&bytes).unwrap_or(Value::Null)
}

async fn body_bytes(resp: axum::response::Response) -> bytes::Bytes {
    axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap()
}

fn multipart_body(boundary: &str, key: &str, content: &[u8]) -> Vec<u8> {
    let mut body = Vec::new();
    // key field
    body.extend_from_slice(
        format!(
            "--{boundary}\r\nContent-Disposition: form-data; name=\"key\"\r\n\r\n{key}\r\n",
            boundary = boundary,
            key = key,
        )
        .as_bytes(),
    );
    // file field
    body.extend_from_slice(
        format!(
            "--{boundary}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"{key}\"\r\n\r\n",
            boundary = boundary,
            key = key,
        )
        .as_bytes(),
    );
    body.extend_from_slice(content);
    body.extend_from_slice(format!("\r\n--{boundary}--\r\n", boundary = boundary).as_bytes());
    body
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests: 501 when no store is configured
// ─────────────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn list_files_returns_501_without_s3() {
    let app = build_server_no_s3().build_router();
    let resp = app
        .oneshot(
            Request::builder()
                .uri("/api/v1/files")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_IMPLEMENTED);
}

#[tokio::test]
async fn download_returns_501_without_s3() {
    let app = build_server_no_s3().build_router();
    let resp = app
        .oneshot(
            Request::builder()
                .uri("/api/v1/files/some-key")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_IMPLEMENTED);
}

#[tokio::test]
async fn presigned_get_returns_501_without_s3() {
    let app = build_server_no_s3().build_router();
    let resp = app
        .oneshot(
            Request::builder()
                .uri("/api/v1/files/key/presigned-get")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_IMPLEMENTED);
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests: list files
// ─────────────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn list_files_empty_store() {
    let app = build_server_with_s3().build_router();
    let resp = app
        .oneshot(
            Request::builder()
                .uri("/api/v1/files")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
    let json = body_json(resp).await;
    assert_eq!(json["total"], 0);
    assert_eq!(json["keys"], serde_json::json!([]));
}

#[tokio::test]
async fn list_files_with_prefix_filter() {
    let store = MockObjectStore::new();
    // Pre-populate
    store.put("uploads/a.txt", b"a".to_vec()).await.unwrap();
    store.put("uploads/b.txt", b"b".to_vec()).await.unwrap();
    store.put("other/c.txt", b"c".to_vec()).await.unwrap();

    let registry = Arc::new(AgentRegistry::new());
    let config = ServerConfig::new()
        .with_cors(false)
        .with_rate_limit(10_000, Duration::from_secs(60));
    let app = GatewayServer::new(config, registry)
        .with_s3(store as Arc<dyn ObjectStore>)
        .build_router();

    let resp = app
        .oneshot(
            Request::builder()
                .uri("/api/v1/files?prefix=uploads/")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
    let json = body_json(resp).await;
    assert_eq!(json["total"], 2);
    let keys: Vec<String> = serde_json::from_value(json["keys"].clone()).unwrap();
    assert!(keys.contains(&"uploads/a.txt".to_string()));
    assert!(keys.contains(&"uploads/b.txt".to_string()));
    assert!(!keys.contains(&"other/c.txt".to_string()));
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests: upload
// ─────────────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn upload_file_creates_object() {
    let app = build_server_with_s3().build_router();
    let boundary = "testboundary123";
    let body = multipart_body(boundary, "test/hello.txt", b"hello world");

    let resp = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/files/upload")
                .header(
                    "content-type",
                    format!("multipart/form-data; boundary={}", boundary),
                )
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::CREATED);
    let json = body_json(resp).await;
    assert_eq!(json["key"], "test/hello.txt");
    assert_eq!(json["size"], 11);
}

#[tokio::test]
async fn upload_missing_key_field_returns_bad_request() {
    let app = build_server_with_s3().build_router();
    let boundary = "b";
    // Only send file field, no key field
    let body = format!(
        "--{b}\r\nContent-Disposition: form-data; name=\"file\"\r\n\r\nhello\r\n--{b}--\r\n",
        b = boundary
    );

    let resp = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/files/upload")
                .header(
                    "content-type",
                    format!("multipart/form-data; boundary={}", boundary),
                )
                .body(Body::from(body.into_bytes()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn upload_empty_key_returns_bad_request() {
    let app = build_server_with_s3().build_router();
    let boundary = "b";
    let body = multipart_body(boundary, "", b"data");

    let resp = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/files/upload")
                .header(
                    "content-type",
                    format!("multipart/form-data; boundary={}", boundary),
                )
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests: download
// ─────────────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn download_existing_file() {
    let store = MockObjectStore::new();
    store
        .put("docs/readme.txt", b"readme content".to_vec())
        .await
        .unwrap();

    let registry = Arc::new(AgentRegistry::new());
    let config = ServerConfig::new()
        .with_cors(false)
        .with_rate_limit(10_000, Duration::from_secs(60));
    let app = GatewayServer::new(config, registry)
        .with_s3(store as Arc<dyn ObjectStore>)
        .build_router();

    let resp = app
        .oneshot(
            Request::builder()
                .uri("/api/v1/files/docs%2Freadme.txt")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = body_bytes(resp).await;
    assert_eq!(bytes.as_ref(), b"readme content");
}

#[tokio::test]
async fn download_missing_file_returns_not_found() {
    let app = build_server_with_s3().build_router();

    let resp = app
        .oneshot(
            Request::builder()
                .uri("/api/v1/files/does-not-exist.txt")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests: delete
// ─────────────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn delete_existing_file_returns_ok() {
    let store = MockObjectStore::new();
    store.put("to-delete.txt", b"bye".to_vec()).await.unwrap();

    let registry = Arc::new(AgentRegistry::new());
    let config = ServerConfig::new()
        .with_cors(false)
        .with_rate_limit(10_000, Duration::from_secs(60));
    let app = GatewayServer::new(config, registry)
        .with_s3(store as Arc<dyn ObjectStore>)
        .build_router();

    let resp = app
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri("/api/v1/files/to-delete.txt")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
    let json = body_json(resp).await;
    assert_eq!(json["status"], "deleted");
}

#[tokio::test]
async fn delete_missing_file_returns_not_found() {
    let app = build_server_with_s3().build_router();

    let resp = app
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri("/api/v1/files/ghost.txt")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests: presigned URLs
// ─────────────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn presigned_get_url_returns_url() {
    let app = build_server_with_s3().build_router();

    let resp = app
        .oneshot(
            Request::builder()
                .uri("/api/v1/files/report.pdf/presigned-get?expires=7200")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
    let json = body_json(resp).await;
    assert_eq!(json["key"], "report.pdf");
    assert_eq!(json["method"], "GET");
    assert!(
        json["url"].as_str().unwrap().contains("report.pdf"),
        "URL should reference the key"
    );
}

#[tokio::test]
async fn presigned_put_url_returns_url() {
    let app = build_server_with_s3().build_router();
    let body = serde_json::json!({ "expires_secs": 1800, "content_type": "image/png" });

    let resp = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/files/photo.png/presigned-put")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
    let json = body_json(resp).await;
    assert_eq!(json["key"], "photo.png");
    assert_eq!(json["method"], "PUT");
    assert_eq!(json["expires_secs"], 1800);
    assert_eq!(json["content_type"], "image/png");
    assert!(
        json["url"].as_str().unwrap().contains("photo.png"),
        "URL should reference the key"
    );
}

#[tokio::test]
async fn presigned_put_uses_default_expires_when_omitted() {
    let app = build_server_with_s3().build_router();
    let body = serde_json::json!({});

    let resp = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/files/file.bin/presigned-put")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
    let json = body_json(resp).await;
    assert_eq!(json["expires_secs"], 3600, "default expiry should be 3600");
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests: existing endpoints unaffected
// ─────────────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn health_endpoint_still_works_with_s3_configured() {
    let app = build_server_with_s3().build_router();

    let resp = app
        .oneshot(
            Request::builder()
                .uri("/health")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests: metadata endpoint
// ─────────────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn metadata_returns_501_without_s3() {
    let app = build_server_no_s3().build_router();
    let resp = app
        .oneshot(
            Request::builder()
                .uri("/api/v1/files/any-key/metadata")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_IMPLEMENTED);
}

#[tokio::test]
async fn metadata_missing_key_returns_not_found() {
    let app = build_server_with_s3().build_router();
    let resp = app
        .oneshot(
            Request::builder()
                .uri("/api/v1/files/does-not-exist.txt/metadata")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn metadata_returns_size_for_existing_file() {
    let store = MockObjectStore::new();
    store
        .put("report.pdf", b"fake pdf content".to_vec())
        .await
        .unwrap();

    let registry = Arc::new(AgentRegistry::new());
    let config = ServerConfig::new()
        .with_cors(false)
        .with_rate_limit(10_000, Duration::from_secs(60));
    let app = GatewayServer::new(config, registry)
        .with_s3(store as Arc<dyn ObjectStore>)
        .build_router();

    let resp = app
        .oneshot(
            Request::builder()
                .uri("/api/v1/files/report.pdf/metadata")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
    let json = body_json(resp).await;
    assert_eq!(json["key"], "report.pdf");
    assert_eq!(
        json["size"], 16,
        "size must match byte count of stored data"
    );
    assert!(
        json["last_modified"].is_string(),
        "last_modified should be a string"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests: file size limits (413 Payload Too Large)
// ─────────────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn upload_exceeds_size_limit_returns_413() {
    // Limit: 10 bytes, body: 11 bytes
    let app = build_server_with_size_limit(10).build_router();
    let boundary = "b";
    let body = multipart_body(boundary, "data.bin", b"hello world"); // 11 bytes

    let resp = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/files/upload")
                .header(
                    "content-type",
                    format!("multipart/form-data; boundary={}", boundary),
                )
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::PAYLOAD_TOO_LARGE);
    let json = body_json(resp).await;
    assert!(
        json["error"].as_str().unwrap_or("").contains("exceeds"),
        "error message should mention size limit"
    );
}

#[tokio::test]
async fn upload_within_size_limit_succeeds() {
    // Limit: 100 bytes, body: 11 bytes
    let app = build_server_with_size_limit(100).build_router();
    let boundary = "b";
    let body = multipart_body(boundary, "data.bin", b"hello world");

    let resp = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/files/upload")
                .header(
                    "content-type",
                    format!("multipart/form-data; boundary={}", boundary),
                )
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::CREATED);
}

#[tokio::test]
async fn upload_exactly_at_size_limit_succeeds() {
    let content = b"1234567890"; // exactly 10 bytes
    let app = build_server_with_size_limit(10).build_router();
    let boundary = "b";
    let body = multipart_body(boundary, "exact.bin", content);

    let resp = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/files/upload")
                .header(
                    "content-type",
                    format!("multipart/form-data; boundary={}", boundary),
                )
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::CREATED);
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests: Content-Type auto-detection
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(feature = "s3")]
#[tokio::test]
async fn upload_returns_content_type_in_response() {
    let app = build_server_with_s3().build_router();
    let boundary = "b";
    let body = multipart_body(boundary, "photo.jpg", b"fake jpeg bytes");

    let resp = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/files/upload")
                .header(
                    "content-type",
                    format!("multipart/form-data; boundary={}", boundary),
                )
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::CREATED);
    let json = body_json(resp).await;
    let ct = json["content_type"].as_str().unwrap_or("");
    assert!(
        ct.contains("image/jpeg") || ct.contains("image/jpg"),
        "expected jpeg MIME type, got: {}",
        ct
    );
}

#[cfg(feature = "s3")]
#[tokio::test]
async fn download_sets_content_type_header_for_png() {
    let store = MockObjectStore::new();
    store.put("image.png", vec![0u8; 4]).await.unwrap();

    let registry = Arc::new(AgentRegistry::new());
    let config = ServerConfig::new()
        .with_cors(false)
        .with_rate_limit(10_000, Duration::from_secs(60));
    let app = GatewayServer::new(config, registry)
        .with_s3(store as Arc<dyn ObjectStore>)
        .build_router();

    let resp = app
        .oneshot(
            Request::builder()
                .uri("/api/v1/files/image.png")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
    let ct = resp
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    assert!(ct.contains("image/png"), "expected image/png, got: {}", ct);
}

#[cfg(feature = "s3")]
#[tokio::test]
async fn download_sets_content_type_header_for_json() {
    let store = MockObjectStore::new();
    store.put("data.json", b"{}".to_vec()).await.unwrap();

    let registry = Arc::new(AgentRegistry::new());
    let config = ServerConfig::new()
        .with_cors(false)
        .with_rate_limit(10_000, Duration::from_secs(60));
    let app = GatewayServer::new(config, registry)
        .with_s3(store as Arc<dyn ObjectStore>)
        .build_router();

    let resp = app
        .oneshot(
            Request::builder()
                .uri("/api/v1/files/data.json")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
    let ct = resp
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    assert!(
        ct.contains("application/json"),
        "expected application/json, got: {}",
        ct
    );
}

#[cfg(feature = "s3")]
#[tokio::test]
async fn presigned_put_auto_detects_content_type_from_extension() {
    let app = build_server_with_s3().build_router();
    // No content_type in body — should be inferred from .pdf extension
    let body = serde_json::json!({ "expires_secs": 3600 });

    let resp = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/files/report.pdf/presigned-put")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
    let json = body_json(resp).await;
    let ct = json["content_type"].as_str().unwrap_or("");
    assert!(
        ct.contains("application/pdf"),
        "expected application/pdf auto-detected, got: {}",
        ct
    );
}
