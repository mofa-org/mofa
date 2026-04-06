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

/// Get the current timestamp in milliseconds since the Unix epoch using
/// `chrono::Utc::now()`, safely avoiding signed-to-unsigned wraparound.
///
/// `chrono::DateTime::timestamp_millis()` returns `i64`. On hosts where the
/// system clock is set before the Unix epoch (misconfigured CI, NTP backward
/// jump, VM clock skew), that value is negative. Casting a negative `i64`
/// to `u64` with `as` wraps it to near `u64::MAX`, silently corrupting any
/// timestamp stored from the result. This function clamps such cases to 0.
///
/// Use this instead of `chrono::Utc::now().timestamp_millis() as u64`.
pub fn chrono_now_ms() -> u64 {
    // try_into() uses TryFrom<i64> for u64, which fails for negative values.
    // We want 0 in that case — not u64::MAX — so we unwrap_or(0).
    chrono::Utc::now()
        .timestamp_millis()
        .try_into()
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn now_ms_returns_reasonable_epoch_millis() {
        let ms = now_ms();
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

    #[test]
    fn chrono_now_ms_returns_reasonable_epoch_millis() {
        let ms = chrono_now_ms();
        assert!(
            ms > 1_577_836_800_000,
            "chrono_now_ms() returned {ms}, expected > 2020-01-01 epoch millis"
        );
    }

    #[test]
    fn chrono_now_ms_is_monotonically_non_decreasing() {
        let a = chrono_now_ms();
        let b = chrono_now_ms();
        assert!(b >= a);
    }

    #[test]
    fn chrono_now_ms_consistent_with_now_ms() {
        let a = now_ms();
        let b = chrono_now_ms();
        let diff = a.abs_diff(b);
        assert!(
            diff < 200,
            "now_ms()={a} and chrono_now_ms()={b} differ by {diff}ms"
        );
    }
}
