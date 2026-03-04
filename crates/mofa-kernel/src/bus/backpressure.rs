//! Backpressure configuration for the agent communication bus.
//!
//! Provides fine-grained control over how the bus handles buffer overflow
//! and slow consumers. Each channel can be independently configured with
//! its own buffer size and lag recovery policy.
//!
//! # Design Rationale
//!
//! The bus uses [`tokio::sync::broadcast`] under the hood for fan-out.
//! Broadcast channels have a fixed-size internal ring buffer; when a slow
//! receiver falls behind, the oldest messages are overwritten and the
//! receiver gets [`tokio::sync::broadcast::error::RecvError::Lagged(n)`].
//!
//! Rather than fighting this model (the failed PR #441 tried to replace
//! broadcast entirely), we embrace it and give callers control over:
//!
//! - **Buffer size** — how many messages to buffer per channel.
//! - **Lag policy** — what to do when a receiver detects it has lagged.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use super::CommunicationMode;

// ---------------------------------------------------------------------------
// Lag Policy
// ---------------------------------------------------------------------------

/// Policy for handling receiver lag (when a slow consumer misses messages).
///
/// Broadcast channels overwrite the oldest buffered messages when full.
/// When a receiver detects it has fallen behind, the lag policy determines
/// the recovery behavior.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub enum LagPolicy {
    /// Return [`BusError::MessageLag(n)`] immediately.
    ///
    /// Callers must handle the error explicitly. Best for systems where
    /// message loss is unacceptable and the caller needs to take corrective
    /// action (e.g., request a full state sync).
    #[default]
    Error,

    /// Log the lag event, increment metrics, and automatically re-try
    /// `recv()` to return the next available message.
    ///
    /// Best for telemetry, monitoring, and non-critical event streams
    /// where occasional message loss is tolerable.
    SkipAndContinue,
}

// ---------------------------------------------------------------------------
// Channel Configuration
// ---------------------------------------------------------------------------

/// Default buffer size for channels. Chosen to balance memory usage against
/// tolerance for short bursts. For high-throughput channels, callers should
/// set a larger value via [`BusConfig`].
pub const DEFAULT_BUFFER_SIZE: usize = 256;

/// Configuration for a single bus channel.
///
/// # Examples
///
/// ```
/// use mofa_kernel::bus::backpressure::{ChannelConfig, LagPolicy};
///
/// // High-throughput telemetry: large buffer, skip lag events
/// let telemetry = ChannelConfig::new(4096).with_lag_policy(LagPolicy::SkipAndContinue);
///
/// // Critical consensus: small buffer, strict lag errors
/// let consensus = ChannelConfig::new(64).with_lag_policy(LagPolicy::Error);
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChannelConfig {
    /// Number of messages the channel can buffer before the oldest is
    /// overwritten. Must be ≥ 1.
    buffer_size: usize,

    /// How to handle receiver lag (missed messages).
    lag_policy: LagPolicy,
}

impl ChannelConfig {
    /// Create a new channel configuration with the given buffer size.
    ///
    /// # Panics
    ///
    /// Panics if `buffer_size` is 0 (broadcast channels require ≥ 1).
    #[must_use]
    pub fn new(buffer_size: usize) -> Self {
        assert!(buffer_size > 0, "buffer_size must be ≥ 1");
        Self {
            buffer_size,
            lag_policy: LagPolicy::default(),
        }
    }

    /// Set the lag policy for this channel.
    #[must_use]
    pub fn with_lag_policy(mut self, policy: LagPolicy) -> Self {
        self.lag_policy = policy;
        self
    }

    /// Returns the configured buffer size.
    #[inline]
    pub fn buffer_size(&self) -> usize {
        self.buffer_size
    }

    /// Returns the configured lag policy.
    #[inline]
    pub fn lag_policy(&self) -> &LagPolicy {
        &self.lag_policy
    }
}

impl Default for ChannelConfig {
    fn default() -> Self {
        Self::new(DEFAULT_BUFFER_SIZE)
    }
}

// ---------------------------------------------------------------------------
// Bus Configuration
// ---------------------------------------------------------------------------

/// Top-level configuration for the [`AgentBus`](super::AgentBus).
///
/// Provides a default channel config plus optional per-mode overrides.
///
/// # Examples
///
/// ```
/// use mofa_kernel::bus::backpressure::{BusConfig, ChannelConfig, LagPolicy};
/// use mofa_kernel::bus::CommunicationMode;
///
/// let config = BusConfig::default()
///     .with_broadcast(ChannelConfig::new(1024))
///     .with_override(
///         CommunicationMode::PubSub("telemetry".into()),
///         ChannelConfig::new(4096).with_lag_policy(LagPolicy::SkipAndContinue),
///     );
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BusConfig {
    /// Default configuration applied to all channels unless overridden.
    default_channel: ChannelConfig,

    /// Configuration for the global broadcast channel.
    broadcast_channel: ChannelConfig,

    /// Per-mode overrides. Takes precedence over `default_channel`.
    #[serde(default)]
    overrides: HashMap<CommunicationMode, ChannelConfig>,
}

impl BusConfig {
    /// Create a bus config with the given default channel config.
    #[must_use]
    pub fn new(default_channel: ChannelConfig) -> Self {
        Self {
            broadcast_channel: default_channel.clone(),
            default_channel,
            overrides: HashMap::new(),
        }
    }

    /// Override the broadcast channel config.
    #[must_use]
    pub fn with_broadcast(mut self, config: ChannelConfig) -> Self {
        self.broadcast_channel = config;
        self
    }

    /// Add a per-mode override.
    #[must_use]
    pub fn with_override(mut self, mode: CommunicationMode, config: ChannelConfig) -> Self {
        self.overrides.insert(mode, config);
        self
    }

    /// Resolve the effective config for a given communication mode.
    ///
    /// Priority: per-mode override > broadcast special case > default.
    pub fn resolve(&self, mode: &CommunicationMode) -> &ChannelConfig {
        if let Some(override_config) = self.overrides.get(mode) {
            return override_config;
        }
        if matches!(mode, CommunicationMode::Broadcast) {
            return &self.broadcast_channel;
        }
        &self.default_channel
    }

    /// Returns the broadcast channel config.
    pub fn broadcast(&self) -> &ChannelConfig {
        &self.broadcast_channel
    }

    /// Returns the default channel config.
    pub fn default_channel(&self) -> &ChannelConfig {
        &self.default_channel
    }
}

impl Default for BusConfig {
    fn default() -> Self {
        Self::new(ChannelConfig::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_channel_config() {
        let config = ChannelConfig::default();
        assert_eq!(config.buffer_size(), DEFAULT_BUFFER_SIZE);
        assert_eq!(*config.lag_policy(), LagPolicy::Error);
    }

    #[test]
    fn custom_channel_config() {
        let config = ChannelConfig::new(512).with_lag_policy(LagPolicy::SkipAndContinue);
        assert_eq!(config.buffer_size(), 512);
        assert_eq!(*config.lag_policy(), LagPolicy::SkipAndContinue);
    }

    #[test]
    #[should_panic(expected = "buffer_size must be ≥ 1")]
    fn zero_buffer_panics() {
        ChannelConfig::new(0);
    }

    #[test]
    fn bus_config_resolve_default() {
        let config = BusConfig::default();
        let resolved = config.resolve(&CommunicationMode::PointToPoint("agent".into()));
        assert_eq!(resolved.buffer_size(), DEFAULT_BUFFER_SIZE);
    }

    #[test]
    fn bus_config_resolve_broadcast() {
        let config = BusConfig::default().with_broadcast(ChannelConfig::new(1024));
        let resolved = config.resolve(&CommunicationMode::Broadcast);
        assert_eq!(resolved.buffer_size(), 1024);
    }

    #[test]
    fn bus_config_resolve_override() {
        let mode = CommunicationMode::PubSub("critical".into());
        let config = BusConfig::default().with_override(mode.clone(), ChannelConfig::new(64));
        let resolved = config.resolve(&mode);
        assert_eq!(resolved.buffer_size(), 64);
    }

    #[test]
    fn bus_config_override_takes_precedence() {
        let config = BusConfig::default()
            .with_broadcast(ChannelConfig::new(1024))
            .with_override(CommunicationMode::Broadcast, ChannelConfig::new(2048));
        let resolved = config.resolve(&CommunicationMode::Broadcast);
        assert_eq!(resolved.buffer_size(), 2048);
    }

    #[test]
    fn channel_config_serialization_roundtrip() {
        let config = ChannelConfig::new(512).with_lag_policy(LagPolicy::SkipAndContinue);
        let json = serde_json::to_string(&config).unwrap();
        let deserialized: ChannelConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(config, deserialized);
    }
}
