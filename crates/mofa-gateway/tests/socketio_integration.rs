//! Integration tests for the Socket.IO gateway integration.
//!
//! Tests that:
//! 1. `GatewayServer::build_router()` succeeds when Socket.IO is configured
//! 2. Normal HTTP endpoints still work when the Socket.IO layer is present
//! 3. The Socket.IO polling path (`/socket.io/?EIO=4&transport=polling`)
//!    returns a response (not 404) when Socket.IO is enabled
//!
//! Run:
//! ```bash
//! cargo test -p mofa-gateway --test socketio_integration --features socketio
//! ```

use axum::body::Body;
use axum::http::{Request, StatusCode};
use mofa_gateway::server::{GatewayServer, ServerConfig};
use mofa_runtime::agent::registry::AgentRegistry;
use std::sync::Arc;
use std::time::Duration;
use tower::ServiceExt;

fn base_config() -> ServerConfig {
    ServerConfig::new()
        .with_host("127.0.0.1")
        .with_port(0)
        .with_cors(false)
        .with_rate_limit(10_000, Duration::from_secs(60))
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests that always run (no socketio feature required)
// ─────────────────────────────────────────────────────────────────────────────

/// Without Socket.IO configured, health endpoint should still work.
#[tokio::test]
async fn health_endpoint_works_without_socketio() {
    let registry = Arc::new(AgentRegistry::new());
    let app = GatewayServer::new(base_config(), registry)
        .build_router();

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

/// Without Socket.IO, requests to the Socket.IO path should return 404.
#[tokio::test]
async fn socketio_polling_path_is_404_without_layer() {
    let registry = Arc::new(AgentRegistry::new());
    let app = GatewayServer::new(base_config(), registry)
        .build_router();

    let resp = app
        .oneshot(
            Request::builder()
                .uri("/socket.io/?EIO=4&transport=polling")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(
        resp.status(),
        StatusCode::NOT_FOUND,
        "Socket.IO endpoint should not exist when layer is not attached"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests that require the `socketio` feature
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(feature = "socketio")]
mod with_socketio {
    use super::*;
    use mofa_integrations::socketio::SocketIoConfig;
    use mofa_kernel::bus::AgentBus;

    fn make_server_with_socketio() -> mofa_gateway::server::GatewayServer {
        let registry = Arc::new(AgentRegistry::new());
        let bus = Arc::new(AgentBus::new());
        let sio_cfg = SocketIoConfig::new().with_auth_token("test-token");
        GatewayServer::new(base_config(), registry)
            .with_socket_io(bus, sio_cfg)
    }

    /// Health endpoint must still respond 200 when the Socket.IO layer is active.
    #[tokio::test]
    async fn health_endpoint_works_with_socketio() {
        let app = make_server_with_socketio().build_router();

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

    /// Agents list endpoint should work with Socket.IO layer.
    #[tokio::test]
    async fn agents_list_works_with_socketio() {
        let app = make_server_with_socketio().build_router();

        let resp = app
            .oneshot(
                Request::builder()
                    .uri("/agents")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        // Should return 200 with an empty list — not 500 or anything Socket.IO-related
        assert_eq!(resp.status(), StatusCode::OK);
    }

    /// Socket.IO polling path must return something other than 404.
    ///
    /// Engine.IO v4 polling handshake: GET /socket.io/?EIO=4&transport=polling
    /// socketioxide intercepts this — it should not fall through to a 404.
    #[tokio::test]
    async fn socketio_polling_path_not_404() {
        let app = make_server_with_socketio().build_router();

        let resp = app
            .oneshot(
                Request::builder()
                    .uri("/socket.io/?EIO=4&transport=polling")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_ne!(
            resp.status(),
            StatusCode::NOT_FOUND,
            "Socket.IO polling path should be handled by the socketioxide layer"
        );
    }

    /// Configuring Socket.IO with a custom namespace should not break HTTP routes.
    #[tokio::test]
    async fn custom_namespace_does_not_break_http_routes() {
        let registry = Arc::new(AgentRegistry::new());
        let bus = Arc::new(AgentBus::new());
        let sio_cfg = SocketIoConfig::new().with_namespace("/gateway");
        let app = GatewayServer::new(base_config(), registry)
            .with_socket_io(bus, sio_cfg)
            .build_router();

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

    /// Upload via HTTP succeeds and returns 201 when both Socket.IO and S3 are
    /// configured. The Socket.IO events are emitted as side-effects but cannot
    /// easily be asserted without a real Socket.IO client; this test ensures the
    /// HTTP path is not broken by the event emission code.
    #[cfg(feature = "s3")]
    #[tokio::test]
    async fn upload_returns_201_with_socketio_and_s3() {
        use async_trait::async_trait;
        use mofa_kernel::ObjectStore;
        use mofa_kernel::agent::error::AgentResult;
        use std::collections::HashMap;
        use tokio::sync::RwLock;

        struct InlineStore(Arc<RwLock<HashMap<String, Vec<u8>>>>);
        impl InlineStore {
            fn new() -> Arc<Self> {
                Arc::new(Self(Arc::new(RwLock::new(HashMap::new()))))
            }
        }
        #[async_trait]
        impl ObjectStore for InlineStore {
            async fn put(&self, k: &str, v: Vec<u8>) -> AgentResult<()> {
                self.0.write().await.insert(k.into(), v);
                Ok(())
            }
            async fn get(&self, k: &str) -> AgentResult<Option<Vec<u8>>> {
                Ok(self.0.read().await.get(k).cloned())
            }
            async fn delete(&self, k: &str) -> AgentResult<bool> {
                Ok(self.0.write().await.remove(k).is_some())
            }
            async fn list_keys(&self, _: &str) -> AgentResult<Vec<String>> {
                Ok(self.0.read().await.keys().cloned().collect())
            }
            async fn presigned_get_url(&self, k: &str, e: u64) -> AgentResult<String> {
                Ok(format!("https://mock/{k}?expires={e}"))
            }
        }

        let registry = Arc::new(AgentRegistry::new());
        let bus = Arc::new(AgentBus::new());
        let sio_cfg = SocketIoConfig::new();
        let app = GatewayServer::new(base_config(), registry)
            .with_socket_io(bus, sio_cfg)
            .with_s3(InlineStore::new() as Arc<dyn mofa_kernel::ObjectStore>)
            .build_router();

        // Build a minimal multipart body
        let boundary = "testbnd";
        let mut body = Vec::new();
        body.extend_from_slice(
            format!(
                "--{b}\r\nContent-Disposition: form-data; name=\"key\"\r\n\r\nuploads/test.txt\r\n",
                b = boundary
            )
            .as_bytes(),
        );
        body.extend_from_slice(
            format!(
                "--{b}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"test.txt\"\r\n\r\n",
                b = boundary
            )
            .as_bytes(),
        );
        body.extend_from_slice(b"hello socket.io");
        body.extend_from_slice(format!("\r\n--{b}--\r\n", b = boundary).as_bytes());

        use axum::body::Body;
        use axum::http::{Request, StatusCode};
        use tower::ServiceExt;

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

        assert_eq!(
            resp.status(),
            StatusCode::CREATED,
            "upload must succeed even when Socket.IO bridge is active"
        );
    }

    /// A file upload that exceeds the size limit returns 413 even when Socket.IO
    /// is configured (the failure event is emitted but HTTP must still reject).
    #[cfg(feature = "s3")]
    #[tokio::test]
    async fn upload_over_size_limit_returns_413_with_socketio() {
        use async_trait::async_trait;
        use mofa_kernel::ObjectStore;
        use mofa_kernel::agent::error::AgentResult;
        use std::collections::HashMap;
        use tokio::sync::RwLock;

        struct InlineStore(Arc<RwLock<HashMap<String, Vec<u8>>>>);
        impl InlineStore {
            fn new() -> Arc<Self> {
                Arc::new(Self(Arc::new(RwLock::new(HashMap::new()))))
            }
        }
        #[async_trait]
        impl ObjectStore for InlineStore {
            async fn put(&self, k: &str, v: Vec<u8>) -> AgentResult<()> {
                self.0.write().await.insert(k.into(), v);
                Ok(())
            }
            async fn get(&self, k: &str) -> AgentResult<Option<Vec<u8>>> {
                Ok(self.0.read().await.get(k).cloned())
            }
            async fn delete(&self, k: &str) -> AgentResult<bool> {
                Ok(self.0.write().await.remove(k).is_some())
            }
            async fn list_keys(&self, _: &str) -> AgentResult<Vec<String>> {
                Ok(Vec::new())
            }
            async fn presigned_get_url(&self, k: &str, e: u64) -> AgentResult<String> {
                Ok(format!("https://mock/{k}?expires={e}"))
            }
        }

        let registry = Arc::new(AgentRegistry::new());
        let bus = Arc::new(AgentBus::new());
        let sio_cfg = SocketIoConfig::new();
        // Limit: 5 bytes
        let config = base_config().with_max_upload_size(5);
        let app = GatewayServer::new(config, registry)
            .with_socket_io(bus, sio_cfg)
            .with_s3(InlineStore::new() as Arc<dyn mofa_kernel::ObjectStore>)
            .build_router();

        let boundary = "b";
        let mut body = Vec::new();
        body.extend_from_slice(
            format!(
                "--{b}\r\nContent-Disposition: form-data; name=\"key\"\r\n\r\nfile.bin\r\n",
                b = boundary
            )
            .as_bytes(),
        );
        body.extend_from_slice(
            format!(
                "--{b}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"file.bin\"\r\n\r\n",
                b = boundary
            )
            .as_bytes(),
        );
        body.extend_from_slice(b"hello world"); // 11 bytes > 5 byte limit
        body.extend_from_slice(format!("\r\n--{b}--\r\n", b = boundary).as_bytes());

        use axum::body::Body;
        use axum::http::{Request, StatusCode};
        use tower::ServiceExt;

        let resp = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/files/upload")
                    .header("content-type", format!("multipart/form-data; boundary={}", boundary))
                    .body(Body::from(body))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(resp.status(), StatusCode::PAYLOAD_TOO_LARGE);
    }

    /// Socket.IO and S3 can both be configured simultaneously.
    ///
    /// Uses a minimal in-memory store to avoid importing across test binaries.
    #[cfg(feature = "s3")]
    #[tokio::test]
    async fn socketio_and_s3_coexist() {
        use async_trait::async_trait;
        use mofa_kernel::ObjectStore;
        use mofa_kernel::agent::error::AgentResult;
        use std::collections::HashMap;
        use tokio::sync::RwLock;

        struct InlineStore(Arc<RwLock<HashMap<String, Vec<u8>>>>);
        impl InlineStore {
            fn new() -> Arc<Self> {
                Arc::new(Self(Arc::new(RwLock::new(HashMap::new()))))
            }
        }
        #[async_trait]
        impl ObjectStore for InlineStore {
            async fn put(&self, k: &str, v: Vec<u8>) -> AgentResult<()> {
                self.0.write().await.insert(k.into(), v);
                Ok(())
            }
            async fn get(&self, k: &str) -> AgentResult<Option<Vec<u8>>> {
                Ok(self.0.read().await.get(k).cloned())
            }
            async fn delete(&self, k: &str) -> AgentResult<bool> {
                Ok(self.0.write().await.remove(k).is_some())
            }
            async fn list_keys(&self, _: &str) -> AgentResult<Vec<String>> {
                Ok(self.0.read().await.keys().cloned().collect())
            }
            async fn presigned_get_url(&self, k: &str, e: u64) -> AgentResult<String> {
                Ok(format!("https://mock/{k}?expires={e}"))
            }
        }

        let registry = Arc::new(AgentRegistry::new());
        let bus = Arc::new(AgentBus::new());
        let sio_cfg = SocketIoConfig::new();
        let app = GatewayServer::new(base_config(), registry)
            .with_socket_io(bus, sio_cfg)
            .with_s3(InlineStore::new() as Arc<dyn mofa_kernel::ObjectStore>)
            .build_router();

        // Health endpoint should work
        let resp = app
            .oneshot(Request::builder().uri("/health").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
    }
}
