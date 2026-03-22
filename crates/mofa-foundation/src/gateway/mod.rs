//! Gateway layer implementations for `mofa-foundation`.
//!
//! Provides concrete types implementing kernel gateway traits:
//! - [`auth`] — [`InMemoryApiKeyStore`](auth::InMemoryApiKeyStore), [`ApiKeyAuthProvider`](auth::ApiKeyAuthProvider)

pub mod auth;

pub use auth::{ApiKeyAuthProvider, InMemoryApiKeyStore};
