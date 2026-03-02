//! Socket.IO bridge for real-time agent event streaming
//!
//! Bridges the kernel `AgentBus` broadcast channel to Socket.IO clients.
//! Clients connect to the `/agents` namespace and receive JSON-encoded
//! `AgentMessage` events in real time.

use axum::Router;
use mofa_kernel::bus::AgentBus;
use mofa_kernel::message::AgentMessage;
use serde::Deserialize;
use serde_json::{Value, json};
use socketioxide::SocketIo;
use socketioxide::extract::{Data, SocketRef};
use std::sync::Arc;
use tokio::sync::broadcast;
use tracing::{debug, info, warn};

// ─────────────────────────────────────────────────────────────────────────────
// Configuration
// ─────────────────────────────────────────────────────────────────────────────

/// Configuration for the Socket.IO bridge
#[derive(Debug, Clone)]
pub struct SocketIoConfig {
    /// Optional bearer token required in the Socket.IO handshake `auth` field.
    pub auth_token: Option<String>,
    /// Socket.IO namespace for agent events (default: `/agents`).
    pub namespace: String,
    /// Internal broadcast channel buffer size.
    pub channel_buffer: usize,
}

impl Default for SocketIoConfig {
    fn default() -> Self {
        Self::new()
    }
}

impl SocketIoConfig {
    pub fn new() -> Self {
        Self {
            auth_token: None,
            namespace: "/agents".to_string(),
            channel_buffer: 256,
        }
    }

    /// Require clients to send `{ auth: { token: "..." } }` in the handshake.
    pub fn with_auth_token(mut self, token: impl Into<String>) -> Self {
        self.auth_token = Some(token.into());
        self
    }

    /// Override the namespace (default: `/agents`).
    pub fn with_namespace(mut self, ns: impl Into<String>) -> Self {
        self.namespace = ns.into();
        self
    }

    /// Override the broadcast channel buffer size.
    pub fn with_buffer(mut self, size: usize) -> Self {
        self.channel_buffer = size;
        self
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Auth payload
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize, Default)]
struct AuthPayload {
    #[serde(default)]
    token: String,
}

// ─────────────────────────────────────────────────────────────────────────────
// Bridge
// ─────────────────────────────────────────────────────────────────────────────

/// Socket.IO server that bridges `AgentBus` broadcast messages to clients.
pub struct SocketIoBridge {
    config: SocketIoConfig,
    bus: Arc<AgentBus>,
}

impl SocketIoBridge {
    pub fn new(config: SocketIoConfig, bus: Arc<AgentBus>) -> Self {
        Self { config, bus }
    }

    /// Build the Socket.IO layer and a companion router.
    ///
    /// Apply the returned `layer` to your axum application.
    pub fn build(self) -> (socketioxide::layer::SocketIoLayer, Router) {
        let auth_token = self.config.auth_token.clone();
        // One clone for the forwarding task, the original consumed by io.ns()
        let namespace_fwd = self.config.namespace.clone();
        let namespace = self.config.namespace.clone();

        let (layer, io) = SocketIo::builder().build_layer();

        // Background task: forward AgentBus broadcasts → Socket.IO
        let mut broadcast_rx: broadcast::Receiver<Vec<u8>> = self.bus.subscribe_broadcast();
        let io_fwd = io.clone();
        let ns_fwd = namespace_fwd;

        tokio::spawn(async move {
            info!("Socket.IO bridge started on namespace '{}'", ns_fwd);
            loop {
                match broadcast_rx.recv().await {
                    Ok(raw) => {
                        let payload = match bincode::deserialize::<AgentMessage>(&raw) {
                            Ok(msg) => agent_message_to_json(&msg),
                            Err(_) => json!({ "raw": hex::encode(&raw) }),
                        };
                        if let Some(ns) = io_fwd.of(&ns_fwd) {
                            let _ = ns.emit("agent_message", &payload);
                        }
                    }
                    Err(broadcast::error::RecvError::Lagged(n)) => {
                        warn!("Socket.IO bridge lagged by {} messages", n);
                    }
                    Err(broadcast::error::RecvError::Closed) => {
                        info!("AgentBus closed; Socket.IO bridge stopping");
                        break;
                    }
                }
            }
        });

        // Namespace connection handler — pass by value for 'static requirement
        io.ns(
            namespace,
            move |socket: SocketRef, Data(auth): Data<AuthPayload>| {
                if let Some(required) = &auth_token {
                    if auth.token != *required {
                        warn!(socket_id = %socket.id, "Socket.IO rejected: bad token");
                        let _ = socket.disconnect();
                        return;
                    }
                }

                info!(socket_id = %socket.id, "Socket.IO client connected");
                let _ = socket.emit("connected", &json!({}));

                // Clients can declare topic interest
                socket.on(
                    "subscribe",
                    |socket: SocketRef, Data(data): Data<Value>| async move {
                        let topic = data
                            .get("topic")
                            .and_then(|v: &Value| v.as_str())
                            .unwrap_or("*");
                        debug!(socket_id = %socket.id, topic, "client subscribed");
                    },
                );

                socket.on_disconnect(|socket: SocketRef| async move {
                    info!(socket_id = %socket.id, "Socket.IO client disconnected");
                });
            },
        );

        (layer, Router::new())
    }
}

fn agent_message_to_json(msg: &AgentMessage) -> Value {
    serde_json::to_value(msg).unwrap_or_else(|_| json!({ "error": "serialization failed" }))
}
