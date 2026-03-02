//! Typed errors for the agent communication bus.

use thiserror::Error;

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

    /// The channel buffer is full and the configured strategy does not
    /// allow dropping messages.
    #[error("Buffer full for channel: {0}")]
    BufferFull(String),

    /// The receiver lagged behind and missed messages. The inner value
    /// is the number of messages that were lost.
    ///
    /// This error is only returned when the channel's [`LagPolicy`](super::backpressure::LagPolicy)
    /// is set to [`Error`](super::backpressure::LagPolicy::Error).
    #[error("Receiver lagged behind, missed {0} message(s)")]
    MessageLag(u64),
}
