//! MoFA Gateway - Control plane and API gateway for agent management
//!
//! This crate provides an HTTP control plane that sits in front of running
//! MoFA agents and exposes a REST API for lifecycle management and request
//! routing.
//!
//! # Endpoints
//!
//! | Method   | Path                       | Description                        |
//! |----------|----------------------------|------------------------------------|
//! | `POST`   | `/agents`                  | Create and register an agent       |
//! | `GET`    | `/agents`                  | List all registered agents         |
//! | `GET`    | `/agents/{id}/status`      | Detailed status for one agent      |
//! | `POST`   | `/agents/{id}/stop`        | Gracefully stop an agent           |
//! | `DELETE` | `/agents/{id}`             | Remove agent from registry         |
//! | `POST`   | `/agents/{id}/chat`        | Send a message and get a response  |
//! | `GET`    | `/health`                  | Liveness probe                     |
//! | `GET`    | `/ready`                   | Readiness probe                    |
//!
//! # Example
//!
//! ```rust,no_run
//! use mofa_gateway::{GatewayServer, GatewayConfig};
//! use mofa_runtime::agent::registry::AgentRegistry;
//! use std::sync::Arc;
//!
//! #[tokio::main]
//! async fn main() {
//!     let registry = Arc::new(AgentRegistry::new());
//!
//!     let config = GatewayConfig::new()
//!         .with_port(8090);
//!
//!     GatewayServer::new(config, registry)
//!         .start()
//!         .await
//!         .unwrap();
//! }
//! ```

pub mod error;
pub mod handlers;
pub mod middleware;
pub mod server;
pub mod state;

pub use error::{GatewayError, GatewayResult};
pub use middleware::RateLimiter;
pub use server::{GatewayConfig, GatewayServer};
pub use state::AppState;
