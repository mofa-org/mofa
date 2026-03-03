pub mod types;
pub mod provider;
pub mod streaming;
/// Streaming Response Protocol — typed stream framing with cancellation and
/// heartbeat support.  Requires the `streaming` Cargo feature.
#[cfg(feature = "streaming")]
pub mod srp;

pub use types::*;
pub use provider::*;
pub use streaming::*;

#[cfg(feature = "streaming")]
pub use srp::{
    stream_inference, InferenceSink, SinkError, SrpConfig, StreamEvent,
};
