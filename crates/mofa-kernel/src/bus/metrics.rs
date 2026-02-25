use std::sync::atomic::{AtomicUsize, Ordering};

/// Event Bus Metrics for monitoring backpressure and drops
#[derive(Debug, Default)]
pub struct EventBusMetrics {
    /// Number of messages dropped due to backpressure
    pub dropped_messages: AtomicUsize,
    /// Number of messages currently buffered
    pub buffer_utilization: AtomicUsize,
}

impl EventBusMetrics {
    pub fn new() -> Self {
        Self::default()
    }

    /// Increment dropped messages counter
    pub fn record_drop(&self) {
        self.dropped_messages.fetch_add(1, Ordering::Relaxed);
    }

    /// Increment buffer utilization
    pub fn increment_buffer(&self) {
        self.buffer_utilization.fetch_add(1, Ordering::Relaxed);
    }

    /// Decrement buffer utilization
    pub fn decrement_buffer(&self) {
        self.buffer_utilization.fetch_sub(1, Ordering::Relaxed);
    }

    /// Get current dropped messages count
    pub fn get_dropped_messages(&self) -> usize {
        self.dropped_messages.load(Ordering::Relaxed)
    }

    /// Get current buffer utilization
    pub fn get_buffer_utilization(&self) -> usize {
        self.buffer_utilization.load(Ordering::Relaxed)
    }
}
