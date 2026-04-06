//! Request handlers for the control-plane API

pub mod agents;
pub mod capability;
pub mod chat;
pub mod health;
pub mod openai;

pub use agents::agents_router;
pub use capability::capability_router;
pub use chat::chat_router;
pub use health::health_router;
pub use openai::openai_router;
