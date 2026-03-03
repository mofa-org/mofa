pub mod types;
pub mod provider;
pub mod streaming;
/// Inference Request Protocol — unified request/response envelope and
/// capability advertisement for all LLM backends.
pub mod irp;

pub use types::*;
pub use provider::*;
pub use streaming::*;
pub use irp::{
    InferenceCapabilities, InferenceProtocol, InferenceRequest, InferenceResponse,
    RequestModality,
};
