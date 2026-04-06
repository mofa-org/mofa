//! Gateway middleware modules

pub mod rate_limit;

#[cfg(feature = "openai-compat")]
pub mod jwt_auth;

pub use rate_limit::RateLimiter;

#[cfg(feature = "openai-compat")]
pub use jwt_auth::{JwtAuth, JwtError, Claims};
