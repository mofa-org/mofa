//! Request handlers for the control-plane API

pub mod agents;
pub mod chat;
pub mod health;
pub mod local_llm;

pub use agents::agents_router;
pub use chat::chat_router;
pub use health::health_router;
