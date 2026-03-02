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
}
