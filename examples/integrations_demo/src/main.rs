//! Integrations demo
//!
//! Demonstrates both integrations from `mofa-integrations`:
//!
//! 1. **Socket.IO bridge** — starts an HTTP server that streams `AgentBus`
//!    broadcast messages to any connected Socket.IO client.
//!
//! 2. **S3 object store** — shows how to instantiate `S3ObjectStore` and
//!    perform basic operations.  The S3 portion only prints the config; it
//!    does NOT make real network calls so the demo can run without AWS
//!    credentials.  Set the environment variables and un-comment the calls
//!    to exercise a real (or MinIO) bucket.
//!
//! # Run
//!
//! ```bash
//! cargo run -p integrations_demo
//! ```
//!
//! Then connect a Socket.IO v4 client to `http://127.0.0.1:3000` on the
//! `/agents` namespace:
//!
//! ```js
//! const { io } = require("socket.io-client");
//! const socket = io("http://localhost:3000/agents", {
//!   auth: { token: "dev-secret" },
//! });
//! socket.on("agent_message", (msg) => console.log("agent event:", msg));
//! ```
//!
//! In another terminal publish a broadcast message:
//!
//! ```bash
//! # The demo publishes a synthetic StateSync event every 5 seconds automatically.
//! ```

use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use axum::{Router, routing::get};
use mofa_integrations::socketio::{SocketIoBridge, SocketIoConfig};
use mofa_kernel::bus::{AgentBus, CommunicationMode};
use mofa_kernel::message::AgentMessage;
use mofa_kernel::agent::types::AgentState;
use tracing::info;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter("info,mofa_integrations=debug")
        .init();

    // ── AgentBus setup ───────────────────────────────────────────────────────
    let bus = Arc::new(AgentBus::new());

    // ── Periodic publisher: simulate agent state-change events ───────────────
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

                // Publish over the broadcast channel so the Socket.IO bridge
                // picks it up and forwards it to all connected clients.
                let _ = bus_pub
                    .send_message("demo-publisher", CommunicationMode::Broadcast, &msg)
                    .await;

                info!(tick, "published synthetic StateSync broadcast");
            }
        });
    }

    // ── Socket.IO bridge ─────────────────────────────────────────────────────
    let config = SocketIoConfig::new()
        .with_auth_token("dev-secret")
        .with_namespace("/agents");

    let bridge = SocketIoBridge::new(config, bus.clone());
    let (socket_layer, _socket_router) = bridge.build();

    // ── Axum app ─────────────────────────────────────────────────────────────
    let app = Router::new()
        .route("/", get(|| async { "MoFA Socket.IO bridge is running" }))
        .layer(socket_layer);

    let addr = "127.0.0.1:3000";
    info!("Integrations demo listening on http://{}", addr);
    info!("Connect a Socket.IO client to http://{}/agents", addr);
    info!("  auth: {{ token: 'dev-secret' }}");
    info!("  listen for 'agent_message' events");

    // ── S3 config preview (no network calls) ─────────────────────────────────
    // To use a real bucket, add the `s3` feature to this example's Cargo.toml
    // and uncomment the block below after setting AWS credentials.
    //
    // use mofa_integrations::s3::{S3Config, S3ObjectStore};
    // use mofa_kernel::ObjectStore;
    // let s3_config = S3Config::new("us-east-1", "my-bucket")
    //     .with_endpoint("http://localhost:9000"); // MinIO
    // let store = S3ObjectStore::new(s3_config).await?;
    // store.put("hello.txt", b"hello from MoFA".to_vec()).await?;
    // let data = store.get("hello.txt").await?;
    // let url = store.presigned_get_url("hello.txt", 3600).await?;
    // info!("presigned URL: {url}");

    info!("S3 adapter: set the `s3` feature + AWS env vars to enable S3 operations");
    info!("  AWS_ACCESS_KEY_ID, AWS_SECRET_ACCESS_KEY, AWS_DEFAULT_REGION");

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
