//! Gateway with Socket.IO + S3 — complete working example
//!
//! Starts `GatewayServer` with:
//! - Real-time Socket.IO bridge on `/agents` namespace
//! - File storage endpoints (`/api/v1/files/**`) backed by AWS S3 or MinIO
//! - All existing gateway endpoints (agents, chat, health)
//!
//! # Configuration (environment variables)
//!
//! | Variable              | Default              | Description                       |
//! |-----------------------|----------------------|-----------------------------------|
//! | `GATEWAY_PORT`        | `8090`               | HTTP listen port                  |
//! | `SOCKETIO_TOKEN`      | *(none)*             | Auth token for Socket.IO clients  |
//! | `AWS_ACCESS_KEY_ID`   | *(required for S3)*  | AWS / MinIO access key            |
//! | `AWS_SECRET_ACCESS_KEY` | *(required for S3)* | AWS / MinIO secret key           |
//! | `S3_REGION`           | `us-east-1`          | AWS region                        |
//! | `S3_BUCKET`           | `mofa-files`         | Target bucket                     |
//! | `S3_ENDPOINT`         | *(none)*             | Custom endpoint for MinIO/LocalStack |
//! | `MAX_UPLOAD_MB`       | *(none — unlimited)* | Max upload file size in megabytes |
//!
//! # Run against MinIO
//!
//! ```bash
//! # Start MinIO
//! docker run -p 9000:9000 -p 9001:9001 \
//!   -e MINIO_ROOT_USER=minioadmin \
//!   -e MINIO_ROOT_PASSWORD=minioadmin \
//!   quay.io/minio/minio server /data --console-address ":9001"
//!
//! # Create bucket
//! mc alias set local http://localhost:9000 minioadmin minioadmin
//! mc mb local/mofa-files
//!
//! # Run the example
//! AWS_ACCESS_KEY_ID=minioadmin \
//! AWS_SECRET_ACCESS_KEY=minioadmin \
//! S3_ENDPOINT=http://localhost:9000 \
//! SOCKETIO_TOKEN=dev-secret \
//! cargo run -p gateway_socketio_s3
//! ```
//!
//! # Connect a Socket.IO client
//!
//! ```js
//! const { io } = require("socket.io-client");
//! const socket = io("http://localhost:8090/agents", {
//!   auth: { token: "dev-secret" },
//! });
//! socket.on("connected", () => console.log("connected!"));
//! socket.on("agent_message", (msg) => console.log("event:", msg));
//! ```
//!
//! # File operations
//!
//! ```bash
//! # Upload (Content-Type is auto-detected from extension)
//! curl -X POST http://localhost:8090/api/v1/files/upload \
//!   -F "key=uploads/hello.txt" \
//!   -F "file=@/tmp/hello.txt"
//!
//! # Get metadata (size, content-type, last-modified) — no download
//! curl "http://localhost:8090/api/v1/files/uploads/hello.txt/metadata"
//!
//! # Download (Content-Type header set from extension)
//! curl "http://localhost:8090/api/v1/files/uploads/hello.txt"
//!
//! # Get presigned download URL (valid 1 hour)
//! curl "http://localhost:8090/api/v1/files/uploads/hello.txt/presigned-get?expires=3600"
//!
//! # Get presigned upload URL (content_type auto-detected when omitted)
//! curl -X POST http://localhost:8090/api/v1/files/uploads/photo.png/presigned-put \
//!   -H "Content-Type: application/json" \
//!   -d '{"expires_secs": 3600}'
//!
//! # List files
//! curl "http://localhost:8090/api/v1/files?prefix=uploads/"
//!
//! # Delete a file
//! curl -X DELETE http://localhost:8090/api/v1/files/uploads/hello.txt
//! ```
//!
//! # Socket.IO upload events
//!
//! When a client uploads a file the Socket.IO bridge emits three events:
//! - `file_upload_started`   — `{ key, size, content_type }`
//! - `file_upload_completed` — `{ key, size, content_type }`
//! - `file_upload_failed`    — `{ key, reason }` (on error or size-limit breach)
//!
//! ```js
//! socket.on("file_upload_started",   (e) => console.log("upload started",   e));
//! socket.on("file_upload_completed", (e) => console.log("upload completed", e));
//! socket.on("file_upload_failed",    (e) => console.error("upload failed",  e));
//! ```

use anyhow::{Context, Result};
use mofa_gateway::server::{GatewayServer, ServerConfig, make_s3_store};
use mofa_integrations::socketio::SocketIoConfig;
use mofa_kernel::bus::{AgentBus, CommunicationMode};
use mofa_kernel::message::AgentMessage;
use mofa_kernel::agent::types::AgentState;
use mofa_runtime::agent::registry::AgentRegistry;
use std::sync::Arc;
use std::time::Duration;
use tracing::{info, warn};

fn env_or(key: &str, default: &str) -> String {
    std::env::var(key).unwrap_or_else(|_| default.to_string())
}

fn env_opt(key: &str) -> Option<String> {
    std::env::var(key).ok()
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter("info,mofa_gateway=debug,mofa_integrations=debug")
        .init();

    // ── Configuration from environment ───────────────────────────────────────
    let port: u16 = env_or("GATEWAY_PORT", "8090")
        .parse()
        .context("GATEWAY_PORT must be a valid port number")?;

    let socketio_token = env_opt("SOCKETIO_TOKEN");
    let s3_region = env_or("S3_REGION", "us-east-1");
    let s3_bucket = env_or("S3_BUCKET", "mofa-files");
    let s3_endpoint = env_opt("S3_ENDPOINT");
    let max_upload_mb: Option<u64> = env_opt("MAX_UPLOAD_MB")
        .and_then(|v| v.parse::<u64>().ok());

    // ── AgentBus ─────────────────────────────────────────────────────────────
    let bus = Arc::new(AgentBus::new());

    // Spawn a task that publishes synthetic agent events every 5 seconds
    // so you can see Socket.IO in action without setting up real agents.
    {
        let bus_pub = bus.clone();
        tokio::spawn(async move {
            let mut tick = 0u64;
            loop {
                tokio::time::sleep(Duration::from_secs(5)).await;
                tick += 1;

                let msg = AgentMessage::StateSync {
                    agent_id: format!("demo-agent-{}", tick % 3),
                    state: if tick % 2 == 0 {
                        AgentState::Running
                    } else {
                        AgentState::Ready
                    },
                };

                let _ = bus_pub
                    .send_message("demo-publisher", CommunicationMode::Broadcast, &msg)
                    .await;

                info!(tick, "published synthetic AgentMessage broadcast");
            }
        });
    }

    // ── Socket.IO config ─────────────────────────────────────────────────────
    let mut sio_cfg = SocketIoConfig::new();
    if let Some(token) = &socketio_token {
        sio_cfg = sio_cfg.with_auth_token(token.clone());
        info!("Socket.IO auth token set (SOCKETIO_TOKEN)");
    } else {
        warn!("SOCKETIO_TOKEN not set — Socket.IO accepts any client without authentication");
    }

    // ── S3 / MinIO store ─────────────────────────────────────────────────────
    info!("Initialising S3 store: region={} bucket={}", s3_region, s3_bucket);
    if let Some(ep) = &s3_endpoint {
        info!("  using custom endpoint: {}", ep);
    }

    let s3_store = make_s3_store(&s3_region, &s3_bucket, s3_endpoint)
        .await
        .map_err(|e| anyhow::anyhow!("failed to initialise S3 object store: {}", e))?;

    info!("S3 store ready");

    // ── GatewayServer ─────────────────────────────────────────────────────────
    let registry = Arc::new(AgentRegistry::new());
    let mut config = ServerConfig::new()
        .with_host("0.0.0.0")
        .with_port(port)
        .with_cors(true);

    if let Some(mb) = max_upload_mb {
        config = config.with_max_upload_size(mb * 1024 * 1024);
        info!("Max upload size: {} MB", mb);
    }

    let server = GatewayServer::new(config, registry)
        .with_socket_io(bus, sio_cfg)
        .with_s3(s3_store);

    info!("MoFA Gateway starting on http://0.0.0.0:{}", port);
    info!("  Socket.IO: connect to http://localhost:{}/agents", port);
    info!("  Files API: http://localhost:{}/api/v1/files", port);
    info!("  Health:    http://localhost:{}/health", port);
    info!("  Agents:    http://localhost:{}/agents", port);

    server.start().await.map_err(|e| anyhow::anyhow!("gateway server exited with error: {}", e))?;
    Ok(())
}
