//! Gateway layer implementations for `mofa-foundation`.
//!
//! Provides concrete types implementing kernel gateway traits:
//! - [`registry`] — [`InMemoryRouteRegistry`](registry::InMemoryRouteRegistry)
//! - [`auth`] — [`InMemoryApiKeyStore`](auth::InMemoryApiKeyStore), [`ApiKeyAuthProvider`](auth::ApiKeyAuthProvider)

pub mod auth;
pub mod registry;

pub use auth::{ApiKeyAuthProvider, InMemoryApiKeyStore};
pub use registry::InMemoryRouteRegistry;
