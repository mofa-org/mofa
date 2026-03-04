//! Stability controls to prevent profile thrashing.
//!
//! When memory pressure oscillates near a threshold, the scheduler could
//! rapidly switch between precision profiles (e.g., f16 ↔ q4). This causes
//! "profile thrashing" — repeated model reloads that waste time and memory.
//!
//! `StabilityControl` prevents this via:
//! - **Cooldown**: minimum interval between profile switches
//! - **Hysteresis**: ignore memory changes smaller than a threshold

use std::time::{Duration, Instant};

/// Controls to prevent rapid profile switching under oscillating memory pressure.
#[derive(Debug, Clone)]
pub struct StabilityControl {
    /// Minimum interval between profile switches.
    cooldown: Duration,
    /// Minimum memory change (MB) to consider significant.
    hysteresis_mb: u64,
    /// Timestamp of the last profile switch.
    last_switch: Option<Instant>,
    /// Last observed memory usage (MB).
    last_reading_mb: Option<u64>,
}

impl Default for StabilityControl {
    fn default() -> Self {
        Self {
            cooldown: Duration::from_secs(5),
            hysteresis_mb: 512,
            last_switch: None,
            last_reading_mb: None,
        }
    }
}

impl StabilityControl {
    /// Create controls with custom cooldown and hysteresis.
    pub fn new(cooldown: Duration, hysteresis_mb: u64) -> Self {
        Self {
            cooldown,
            hysteresis_mb,
            last_switch: None,
            last_reading_mb: None,
        }
    }

    /// Can we switch profiles now? (respects cooldown)
    pub fn can_switch(&self) -> bool {
        match self.last_switch {
            Some(ts) => ts.elapsed() >= self.cooldown,
            None => true,
        }
    }

    /// Record that a profile switch just happened.
    pub fn record_switch(&mut self) {
        self.last_switch = Some(Instant::now());
    }

    /// Is the memory change since the last reading significant?
    pub fn is_significant_change(&self, current_mb: u64) -> bool {
        match self.last_reading_mb {
            Some(prev) => current_mb.abs_diff(prev) >= self.hysteresis_mb,
            None => true,
        }
    }

    /// Update the last memory reading.
    pub fn update_reading(&mut self, usage_mb: u64) {
        self.last_reading_mb = Some(usage_mb);
    }

    /// Get the cooldown duration.
    pub fn cooldown(&self) -> Duration {
        self.cooldown
    }

    /// Get the hysteresis threshold.
    pub fn hysteresis_mb(&self) -> u64 {
        self.hysteresis_mb
    }
}
