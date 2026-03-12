pub mod types;
pub mod provider;
pub mod streaming;
/// Streaming Response Protocol — typed stream framing with cancellation and
/// heartbeat support.
pub mod srp;

pub use types::*;
pub use provider::*;
pub use streaming::*;

pub use srp::{
    InferenceSink, SinkError, SrpConfig, StreamEvent,
};
