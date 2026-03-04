//! Lock-free metrics for the agent communication bus.
//!
//! All counters use [`AtomicU64`] with [`Ordering::Relaxed`] — we don't
//! need sequential consistency for monotonic counters, and relaxed ordering
//! avoids unnecessary memory fences on ARM/weak-memory architectures.
//!
//! For point-in-time snapshots suitable for logging or export, call
//! [`BusMetrics::snapshot()`].

use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicU64, Ordering};

/// Ordering used for all metric updates. Relaxed is sufficient because
/// metrics are monotonic counters with no causal dependencies.
const ORD: Ordering = Ordering::Relaxed;

// ---------------------------------------------------------------------------
// Live Metrics (atomic, lock-free)
// ---------------------------------------------------------------------------

/// Real-time, lock-free metrics for the [`AgentBus`](super::AgentBus).
///
/// All fields are atomic counters that can be read from any thread without
/// locking. For a serializable point-in-time view, use [`snapshot()`](Self::snapshot).
///
/// # Thread Safety
///
/// `BusMetrics` is `Send + Sync` by construction (only contains atomics).
/// It is intended to be wrapped in an `Arc` and shared across the bus.
#[derive(Debug, Default)]
pub struct BusMetrics {
    /// Total messages successfully sent across all channels.
    messages_sent: AtomicU64,

    /// Total messages successfully received across all channels.
    messages_received: AtomicU64,

    /// Total messages lost due to receiver lag (slow consumers).
    messages_dropped: AtomicU64,

    /// Number of times a receiver detected lag (may span multiple messages).
    lag_events: AtomicU64,

    /// Number of send operations that failed (channel closed, serialization, etc.).
    send_errors: AtomicU64,

    /// Number of receive operations that failed (excluding lag, which is counted separately).
    receive_errors: AtomicU64,
}

impl BusMetrics {
    /// Create a new zeroed metrics instance.
    pub fn new() -> Self {
        Self::default()
    }

    // -- Increment helpers (called by AgentBus internals) ---------------------

    /// Record a successful send.
    #[inline]
    pub(crate) fn record_send(&self) {
        self.messages_sent.fetch_add(1, ORD);
    }

    /// Record a successful receive.
    #[inline]
    pub(crate) fn record_receive(&self) {
        self.messages_received.fetch_add(1, ORD);
    }

    /// Record `n` messages lost due to receiver lag.
    #[inline]
    pub(crate) fn record_lag(&self, missed: u64) {
        self.messages_dropped.fetch_add(missed, ORD);
        self.lag_events.fetch_add(1, ORD);
    }

    /// Record a send error.
    #[inline]
    pub(crate) fn record_send_error(&self) {
        self.send_errors.fetch_add(1, ORD);
    }

    /// Record a receive error.
    #[inline]
    pub(crate) fn record_receive_error(&self) {
        self.receive_errors.fetch_add(1, ORD);
    }

    // -- Read accessors (public) ----------------------------------------------

    /// Total messages sent.
    #[inline]
    pub fn messages_sent(&self) -> u64 {
        self.messages_sent.load(ORD)
    }

    /// Total messages received.
    #[inline]
    pub fn messages_received(&self) -> u64 {
        self.messages_received.load(ORD)
    }

    /// Total messages dropped due to lag.
    #[inline]
    pub fn messages_dropped(&self) -> u64 {
        self.messages_dropped.load(ORD)
    }

    /// Number of lag events detected.
    #[inline]
    pub fn lag_events(&self) -> u64 {
        self.lag_events.load(ORD)
    }

    /// Number of send errors.
    #[inline]
    pub fn send_errors(&self) -> u64 {
        self.send_errors.load(ORD)
    }

    /// Number of receive errors.
    #[inline]
    pub fn receive_errors(&self) -> u64 {
        self.receive_errors.load(ORD)
    }

    /// Take a consistent-ish snapshot of all counters.
    ///
    /// Note: individual reads are atomic, but the snapshot as a whole is
    /// **not** transactional — concurrent updates between reads may cause
    /// slight inconsistencies. This is expected and acceptable for metrics.
    pub fn snapshot(&self) -> MetricsSnapshot {
        MetricsSnapshot {
            messages_sent: self.messages_sent(),
            messages_received: self.messages_received(),
            messages_dropped: self.messages_dropped(),
            lag_events: self.lag_events(),
            send_errors: self.send_errors(),
            receive_errors: self.receive_errors(),
        }
    }
}

// ---------------------------------------------------------------------------
// Snapshot (serializable)
// ---------------------------------------------------------------------------

/// A serializable point-in-time snapshot of bus metrics.
///
/// Suitable for logging, monitoring endpoints, or export to external
/// observability systems.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MetricsSnapshot {
    pub messages_sent: u64,
    pub messages_received: u64,
    pub messages_dropped: u64,
    pub lag_events: u64,
    pub send_errors: u64,
    pub receive_errors: u64,
}

impl MetricsSnapshot {
    /// Returns the message delivery rate as `received / sent`.
    ///
    /// Returns `1.0` if no messages have been sent (avoid divide-by-zero).
    pub fn delivery_rate(&self) -> f64 {
        if self.messages_sent == 0 {
            return 1.0;
        }
        self.messages_received as f64 / self.messages_sent as f64
    }

    /// Returns the message loss rate as `dropped / sent`.
    ///
    /// Returns `0.0` if no messages have been sent.
    pub fn loss_rate(&self) -> f64 {
        if self.messages_sent == 0 {
            return 0.0;
        }
        self.messages_dropped as f64 / self.messages_sent as f64
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn metrics_start_at_zero() {
        let m = BusMetrics::new();
        let s = m.snapshot();
        assert_eq!(s.messages_sent, 0);
        assert_eq!(s.messages_received, 0);
        assert_eq!(s.messages_dropped, 0);
        assert_eq!(s.lag_events, 0);
        assert_eq!(s.send_errors, 0);
        assert_eq!(s.receive_errors, 0);
    }

    #[test]
    fn record_and_read() {
        let m = BusMetrics::new();
        m.record_send();
        m.record_send();
        m.record_receive();
        m.record_lag(5);
        m.record_send_error();

        assert_eq!(m.messages_sent(), 2);
        assert_eq!(m.messages_received(), 1);
        assert_eq!(m.messages_dropped(), 5);
        assert_eq!(m.lag_events(), 1);
        assert_eq!(m.send_errors(), 1);
        assert_eq!(m.receive_errors(), 0);
    }

    #[test]
    fn snapshot_serialization() {
        let m = BusMetrics::new();
        m.record_send();
        m.record_receive();
        let snap = m.snapshot();
        let json = serde_json::to_string(&snap).unwrap();
        let deserialized: MetricsSnapshot = serde_json::from_str(&json).unwrap();
        assert_eq!(snap, deserialized);
    }

    #[test]
    fn delivery_rate_no_sends() {
        let snap = MetricsSnapshot {
            messages_sent: 0,
            messages_received: 0,
            messages_dropped: 0,
            lag_events: 0,
            send_errors: 0,
            receive_errors: 0,
        };
        assert_eq!(snap.delivery_rate(), 1.0);
        assert_eq!(snap.loss_rate(), 0.0);
    }

    #[test]
    fn delivery_and_loss_rates() {
        let snap = MetricsSnapshot {
            messages_sent: 100,
            messages_received: 90,
            messages_dropped: 10,
            lag_events: 2,
            send_errors: 0,
            receive_errors: 0,
        };
        assert!((snap.delivery_rate() - 0.9).abs() < f64::EPSILON);
        assert!((snap.loss_rate() - 0.1).abs() < f64::EPSILON);
    }
}
