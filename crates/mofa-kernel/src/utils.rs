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
