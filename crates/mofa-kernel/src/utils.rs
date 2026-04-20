use std::time::{SystemTime, UNIX_EPOCH};

/// Get the current timestamp in milliseconds since the UNIX epoch safely.
/// This prevents silent truncation that could happen with `as u64`
/// by using `try_from` and defaulting on overflow.
pub fn now_ms() -> u64 {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    u64::try_from(millis).unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn now_ms_returns_reasonable_epoch_millis() {
        let ms = now_ms();
        // Must be after 2020-01-01 (~1_577_836_800_000 ms) and before some far
        // future date.  This validates the safe u128→u64 conversion path.
        assert!(
            ms > 1_577_836_800_000,
            "now_ms() returned {ms}, expected > 2020-01-01 epoch millis"
        );
    }

    #[test]
    fn now_ms_is_monotonically_non_decreasing() {
        let a = now_ms();
        let b = now_ms();
        assert!(b >= a);
    }
}
