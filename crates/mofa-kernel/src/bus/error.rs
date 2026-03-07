//! Typed errors and result aliases for the agent communication bus.

use error_stack::Report;
use thiserror::Error;

/// Error-stack–backed result alias for bus operations.
///
/// Equivalent to `Result<T, error_stack::Report<BusError>>`.
pub type BusResult<T> = ::std::result::Result<T, Report<BusError>>;

/// Extension trait to convert `Result<T, BusError>` into [`BusResult<T>`].
pub trait IntoBusReport<T> {
    /// Wrap the error in an `error_stack::Report`.
    fn into_report(self) -> BusResult<T>;
}

impl<T> IntoBusReport<T> for ::std::result::Result<T, BusError> {
    #[inline]
    fn into_report(self) -> BusResult<T> {
        self.map_err(Report::new)
    }
}

/// Errors that can occur on the agent communication bus.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum BusError {
    /// The target channel was not found for an agent.
    #[error("Channel not found for agent: {0}")]
    ChannelNotFound(String),

    /// The target agent has not been registered on the bus.
    #[error("Agent not registered: {0}")]
    AgentNotRegistered(String),

    /// Sending a message through a bus channel failed.
    #[error("Send failed: {0}")]
    SendFailed(String),

    /// A serialization or deserialization error occurred.
    #[error("Serialization error: {0}")]
    Serialization(String),
}
