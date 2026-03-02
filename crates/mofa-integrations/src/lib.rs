//! MoFA Integrations - external service adapters for the MoFA agent framework
//!
//! Both integrations are opt-in via Cargo feature flags so they don't add
//! dependencies to projects that don't need them.
//!
//! # Features
//!
//! | Feature    | What it enables |
//! |------------|-----------------|
//! | `socketio` | Real-time Socket.IO bridge between `AgentBus` and WebSocket clients |
//! | `s3`       | AWS S3 / MinIO object-storage adapter implementing `ObjectStore` |
//!
//! # Socket.IO quick-start
//!
//! ```rust,no_run
//! # #[cfg(feature = "socketio")]
//! # async fn _doc() {
//! use mofa_integrations::socketio::{SocketIoConfig, SocketIoBridge};
//! use mofa_kernel::bus::AgentBus;
//! use std::sync::Arc;
//!
//! let bus = Arc::new(AgentBus::new());
//! let config = SocketIoConfig::new().with_auth_token("secret");
//! let bridge = SocketIoBridge::new(config, bus);
//! let (layer, router_fn) = bridge.build();
//! // merge `layer` and the router into your axum app
//! # }
//! ```
//!
//! # S3 quick-start
//!
//! ```rust,no_run
//! # #[cfg(feature = "s3")]
//! # async fn _doc() {
//! use mofa_integrations::s3::{S3Config, S3ObjectStore};
//! use mofa_kernel::ObjectStore;
//!
//! let config = S3Config::new("us-east-1", "my-bucket");
//! let store = S3ObjectStore::new(config).await.unwrap();
//! store.put("report.txt", b"hello".to_vec()).await.unwrap();
//! # }
//! ```

#[cfg(feature = "socketio")]
pub mod socketio;

#[cfg(feature = "s3")]
pub mod s3;
