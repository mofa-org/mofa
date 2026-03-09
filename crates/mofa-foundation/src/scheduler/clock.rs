//! Concrete clock implementations.
//!
//! The [`Clock`] trait is defined in `mofa-kernel` (kernel = traits only).
//! Per MoFA's microkernel rules, concrete structs live here in `mofa-foundation`.

use mofa_kernel::scheduler::Clock;

/// The default [`Clock`] implementation backed by the real system clock.
///
/// Inject a fake clock in tests to make timing-sensitive code deterministic.
/// See INSTRUCTIONS.md §IV.3 — "Timestamp generation logic MUST be abstracted".
pub struct SystemClock;

impl Clock for SystemClock {
    fn now_millis(&self) -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis()
            .try_into()
            .unwrap_or(u64::MAX)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn system_clock_returns_nonzero_millis() {
        let clock = SystemClock;
        let ts = clock.now_millis();
        // Must be after 2020-01-01 (1_577_836_800_000 ms)
        assert!(ts > 1_577_836_800_000, "timestamp looks too old: {ts}");
    }

    #[test]
    fn system_clock_advances_monotonically() {
        let clock = SystemClock;
        let t1 = clock.now_millis();
        let t2 = clock.now_millis();
        assert!(t2 >= t1, "clock went backwards: {t1} > {t2}");
    }
}
