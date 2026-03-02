//! Backend module.

mod openai;
mod registry;

pub use openai::OpenAiBackend;
pub use registry::InMemoryCapabilityRegistry;
