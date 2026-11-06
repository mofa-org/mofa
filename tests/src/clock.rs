//! Deterministic clock for testing time-dependent agent code.

use std::sync::RwLock;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

/// Trait for injectable time sources.
pub trait Clock: Send + Sync {
    /// Returns the current time in milliseconds since an arbitrary epoch.
    fn now_millis(&self) -> u64;

    /// Returns the current time without triggering auto-advance or mutations.
    fn peek_millis(&self) -> u64 {
        self.now_millis()
    }
}

/// A mock clock with manually controlled time.
///
/// Time starts at zero and only advances when you call [`advance`](Self::advance),
/// [`set`](Self::set), or enable [`set_auto_advance`](Self::set_auto_advance).
pub struct MockClock {
    current_ms: AtomicU64,
    auto_advance_ms: RwLock<Option<u64>>,
}

impl Default for MockClock {
    fn default() -> Self {
        Self::new()
    }
}

impl MockClock {
    /// Create a clock starting at time zero.
    pub fn new() -> Self {
        Self {
            current_ms: AtomicU64::new(0),
            auto_advance_ms: RwLock::new(None),
        }
    }

    /// Create a clock starting at the given duration from epoch.
    pub fn starting_at(start: Duration) -> Self {
        Self {
            current_ms: AtomicU64::new(start.as_millis() as u64),
            auto_advance_ms: RwLock::new(None),
        }
    }

    /// Advance the clock by the given duration.
    pub fn advance(&self, duration: Duration) {
        self.current_ms
            .fetch_add(duration.as_millis() as u64, Ordering::Relaxed);
    }

    /// Set the clock to an exact time.
    pub fn set(&self, duration: Duration) {
        self.current_ms
            .store(duration.as_millis() as u64, Ordering::Relaxed);
    }

    /// Advance time forward to an exact target duration if `target` is in the future.
    /// Does nothing if target is in the past or equal to current time.
    pub fn advance_to(&self, target: Duration) {
        let target_ms = target.as_millis() as u64;
        let mut current = self.current_ms.load(Ordering::Relaxed);
        while target_ms > current {
            match self.current_ms.compare_exchange_weak(
                current,
                target_ms,
                Ordering::Relaxed,
                Ordering::Relaxed,
            ) {
                Ok(_) => break,
                Err(actual) => current = actual,
            }
        }
    }

    /// Advance the clock until the given deadline duration is reached.
    pub fn advance_until_deadline(&self, deadline: Duration) {
        self.advance_to(deadline);
    }

    /// Enable auto-advance: each call to [`now_millis`](Clock::now_millis)
    /// automatically moves time forward by the given step.
    pub fn set_auto_advance(&self, step: Duration) {
        *self.auto_advance_ms.write().expect("lock poisoned") = Some(step.as_millis() as u64);
    }

    /// Disable auto-advance.
    pub fn clear_auto_advance(&self) {
        *self.auto_advance_ms.write().expect("lock poisoned") = None;
    }

    /// Peek at current time in milliseconds without triggering auto-advance.
    pub fn peek_millis(&self) -> u64 {
        self.current_ms.load(Ordering::Relaxed)
    }

    /// Returns current time as a [`Duration`] since epoch, triggering auto-advance if enabled.
    pub fn now(&self) -> Duration {
        Duration::from_millis(self.now_millis())
    }

    /// Peek at current time as a [`Duration`] without triggering auto-advance.
    pub fn peek(&self) -> Duration {
        Duration::from_millis(self.peek_millis())
    }

    /// Calculate a deadline [`Duration`] by adding `timeout` to the current (peeked) time.
    pub fn deadline_from_now(&self, timeout: Duration) -> Duration {
        self.peek().saturating_add(timeout)
    }

    /// Check whether the specified deadline has been reached or passed (non-advancing).
    pub fn is_deadline_passed(&self, deadline: Duration) -> bool {
        self.peek() >= deadline
    }

    /// Return remaining duration until deadline with a zero-floor guarantee (saturates at 0).
    pub fn remaining_duration(&self, deadline: Duration) -> Duration {
        let current = self.peek();
        deadline.saturating_sub(current)
    }
}

impl Clock for MockClock {
    fn now_millis(&self) -> u64 {
        let current = self.current_ms.load(Ordering::Relaxed);
        if let Some(step) = *self.auto_advance_ms.read().expect("lock poisoned") {
            self.current_ms.fetch_add(step, Ordering::Relaxed);
        }
        current
    }

    fn peek_millis(&self) -> u64 {
        self.current_ms.load(Ordering::Relaxed)
    }
}

/// A real-time clock backed by [`std::time::SystemTime`].
pub struct SystemClock;

impl Clock for SystemClock {
    fn now_millis(&self) -> u64 {
        use std::time::{SystemTime, UNIX_EPOCH};
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64
    }
}
