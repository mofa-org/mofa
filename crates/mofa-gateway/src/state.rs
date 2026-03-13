//! Shared application state for the control-plane server

use crate::middleware::RateLimiter;
use mofa_kernel::ObjectStore;
use mofa_runtime::agent::registry::AgentRegistry;
use std::sync::Arc;

/// State shared across all request handlers
#[derive(Clone)]
pub struct AppState {
    /// Agent registry - source of truth for all running agents
    pub registry: Arc<AgentRegistry>,
    /// Per-client rate limiter
    pub rate_limiter: Arc<RateLimiter>,
    /// Optional object store for file upload/download endpoints.
    ///
    /// Held as `Arc<dyn ObjectStore>` (the kernel trait) so that `AppState`
    /// remains decoupled from any concrete storage implementation.
    /// `None` when the `s3` feature is disabled or no store was configured.
    pub s3: Option<Arc<dyn ObjectStore>>,
    /// Maximum allowed upload size in bytes.
    ///
    /// Uploads that exceed this limit are rejected with `413 Payload Too Large`.
    /// `None` means no limit is enforced.
    pub max_upload_bytes: Option<u64>,
    /// Socket.IO handle + namespace used to emit real-time upload events.
    ///
    /// Only present when the `socketio` feature is enabled and a Socket.IO
    /// bridge was configured via [`GatewayServer::with_socket_io`].
    #[cfg(feature = "socketio")]
    pub socketio: Option<(socketioxide::SocketIo, String)>,
}

impl AppState {
    /// Create a new `AppState` wrapping the given `AgentRegistry`.
    pub fn new(
        registry: Arc<AgentRegistry>,
        rate_limiter: Arc<RateLimiter>,
        s3: Option<Arc<dyn ObjectStore>>,
    ) -> Self {
        Self {
            registry,
            rate_limiter,
            s3,
            max_upload_bytes: None,
            #[cfg(feature = "socketio")]
            socketio: None,
        }
    }

    /// Set the maximum allowed upload size in bytes.
    pub fn with_max_upload_bytes(mut self, max_bytes: u64) -> Self {
        self.max_upload_bytes = Some(max_bytes);
        self
    }

    /// Attach a Socket.IO handle for server-side event emission.
    #[cfg(feature = "socketio")]
    pub fn with_socketio(mut self, io: socketioxide::SocketIo, namespace: impl Into<String>) -> Self {
        self.socketio = Some((io, namespace.into()));
        self
    }
}
