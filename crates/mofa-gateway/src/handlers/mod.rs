//! Request handlers for the control-plane API

pub mod agents;
pub mod chat;
pub mod health;
pub mod local_llm;
pub mod metrics_handler;
pub mod openai;

pub use agents::agents_router;
pub use chat::chat_router;
pub use health::health_router;
pub use local_llm::*;
pub use metrics_handler::metrics_router;
pub use openai::openai_router;
