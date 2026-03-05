//! `mofa-gateway` — MoFA Cognitive Gateway runtime.
//!
//! This crate provides the concrete implementations of the gateway kernel
//! contracts defined in `mofa-kernel::gateway`:
//!
//! | Kernel contract | Implementation |
//! |----------------|----------------|
//! | [`GatewayRouter`] | [`router::TrieRouter`] |
//! | [`CapabilityRegistry`] | [`backend::InMemoryCapabilityRegistry`] |
//! | [`GatewayFilter`] | [`filter::ApiKeyFilter`], [`filter::RateLimitFilter`], [`filter::LoggingFilter`] |
//!
//! The [`server::GatewayServer`] wires everything together into an axum HTTP
//! service.
//!
//! # Quick start
//!
//! ```rust,no_run
//! use mofa_gateway::server::{GatewayServer, GatewayServerConfig};
//! use mofa_kernel::gateway::{
//!     BackendKind, CapabilityDescriptor, GatewayConfig, RouteConfig,
//! };
//!
//! #[tokio::main]
//! async fn main() {
//!     let gateway_config = GatewayConfig::new("my-gateway")
//!         .with_backend(CapabilityDescriptor::new(
//!             "openai",
//!             BackendKind::LlmOpenAI,
//!             "https://api.openai.com",
//!         ))
//!         .with_route(RouteConfig::new(
//!             "chat",
//!             "/v1/chat/completions",
//!             "openai",
//!         ));
//!
//!     let server = GatewayServer::new(GatewayServerConfig {
//!         port: 3000,
//!         openai_api_key: std::env::var("OPENAI_API_KEY").ok(),
//!         ..Default::default()
//!     });
//!
//!     server.start(gateway_config).await.unwrap();
//! }
//! ```

pub mod backend;
pub mod error;
pub mod filter;
pub mod router;
pub mod server;

// Re-export the kernel gateway types for convenience.
pub use mofa_kernel::gateway;
