//! Memory budget tracking for the scheduler.

/// Tracks memory allocation and availability.
#[derive(Debug, Clone)]
pub struct MemoryBudget {
    /// Total capacity in MB.
    capacity_mb: u64,
    /// Currently used memory in MB.
    used_mb: u64,
    /// Reserved memory (system overhead) in MB.
    reserved_mb: u64,
}

impl MemoryBudget {
    /// Create a new budget with the given total capacity.
    pub fn new(capacity_mb: u64) -> Self {
        Self {
            capacity_mb,
            used_mb: 0,
            reserved_mb: 0,
        }
    }

    /// Total capacity.
    pub fn capacity_mb(&self) -> u64 {
        self.capacity_mb
    }

    /// Currently used memory.
    pub fn used_mb(&self) -> u64 {
        self.used_mb
    }

    /// Available memory (capacity − used − reserved).
    pub fn available_mb(&self) -> u64 {
        self.capacity_mb
            .saturating_sub(self.used_mb)
            .saturating_sub(self.reserved_mb)
    }

    /// Usage as a percentage (0.0–100.0).
    pub fn usage_percent(&self) -> f64 {
        if self.capacity_mb == 0 {
            return 0.0;
        }
        (self.used_mb as f64 / self.capacity_mb as f64) * 100.0
    }

    /// Allocate memory. Returns `true` if successful.
    pub fn allocate(&mut self, amount_mb: u64) -> bool {
        if self.available_mb() >= amount_mb {
            self.used_mb += amount_mb;
            true
        } else {
            false
        }
    }

    /// Release previously allocated memory.
    pub fn release(&mut self, amount_mb: u64) {
        self.used_mb = self.used_mb.saturating_sub(amount_mb);
    }

    /// Reserve memory for system overhead.
    pub fn reserve(&mut self, amount_mb: u64) {
        self.reserved_mb = self.reserved_mb.saturating_add(amount_mb);
    }
}
