//! HTTP proxy module for forwarding requests to backend services.
//!
//! This module provides server-side proxy functionality, allowing the gateway
//! to forward HTTP requests to backend services like mofa-local-llm.

pub mod config;
pub mod handler;
pub mod local_llm;

pub use config::ProxyBackend;
pub use handler::ProxyHandler;
pub use local_llm::LocalLLMBackend;
