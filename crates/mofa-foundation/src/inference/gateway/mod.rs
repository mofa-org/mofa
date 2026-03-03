//! OpenAI-compatible HTTP gateway for the MoFA inference stack.
//!
//! Exposes a `POST /v1/chat/completions` endpoint that accepts OpenAI-spec
//! Chat Completions requests and routes them through the
//! [`crate::inference::InferenceOrchestrator`] using the configured
//! [`crate::inference::RoutingPolicy`].
//!
//! # Quick Start
//!
//! ```rust,no_run
//! use mofa_foundation::inference::gateway::{GatewayConfig, GatewayServer};
//!
//! #[tokio::main]
//! async fn main() {
//!     let config = GatewayConfig::default()
//!         .with_port(8080)
//!         .with_rpm(120);
//!
//!     GatewayServer::new(config).serve().await.unwrap();
//! }
//! ```
//!
//! # Endpoints
//!
//! | Method | Path | Description |
//! |--------|------|-------------|
//! | `POST` | `/v1/chat/completions` | OpenAI-compatible chat completions |
//! | `GET`  | `/v1/models`           | List available models |
//!
//! # Response Headers
//!
//! | Header | Description |
//! |--------|-------------|
//! | `X-MoFA-Backend` | Which backend handled the request (e.g., `local(qwen3)`) |
//! | `X-MoFA-Latency-Ms` | End-to-end orchestrator latency in milliseconds |

pub mod handler;
pub mod rate_limiter;
pub mod server;
pub mod types;

pub use server::GatewayServer;
pub use types::{
    ChatCompletionRequest, ChatCompletionResponse, GatewayConfig, GatewayErrorBody,
    ModelListResponse,
};
