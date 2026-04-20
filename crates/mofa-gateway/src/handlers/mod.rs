//! Request handlers for the control-plane API

pub mod agents;
pub mod chat;
pub mod files;
pub mod health;
pub mod invocation;
pub mod local_llm;
pub mod openai;

pub use agents::agents_router;
pub use chat::chat_router;
pub use files::files_router;
pub use health::health_router;
pub use invocation::InvocationRouter;
pub use local_llm::*;
pub use openai::openai_router;
