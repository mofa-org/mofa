//! Environment variable handling utilities

use std::env as std_env;

/// Get environment variable with a default value
pub fn get_env_var(key: &str, default: &str) -> String {
    std_env::var(key).unwrap_or_else(|_| default.to_string())
}

/// Get environment variable as optional
pub fn get_env_var_opt(key: &str) -> Option<String> {
    std_env::var(key).ok()
}

/// Check if an environment variable is set
pub fn has_env_var(key: &str) -> bool {
    std_env::var(key).is_ok()
}

/// Parse environment variable as boolean
/// Supports: 1, true, yes, on (case-insensitive)
pub fn get_env_bool(key: &str, default: bool) -> bool {
    match std_env::var(key) {
        Ok(val) => {
            let lower = val.to_lowercase();
            matches!(lower.as_str(), "1" | "true" | "yes" | "on")
        }
        Err(_) => default,
    }
}

/// Parse environment variable as integer
pub fn get_env_int(key: &str, default: i64) -> i64 {
    std_env::var(key)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}

/// Parse environment variable as unsigned integer
pub fn get_env_uint(key: &str, default: u64) -> u64 {
    std_env::var(key)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}

/// Set environment variable for current process
pub fn set_env_var(key: &str, value: &str) {
    // SAFETY: set_var is safe for this use case
    unsafe { std_env::set_var(key, value) };
}

/// Remove environment variable for current process
pub fn remove_env_var(key: &str) {
    // SAFETY: remove_var is safe for this use case
    unsafe { std_env::remove_var(key) };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_env_var_default() {
        let result = get_env_var("NONEXISTENT_VAR_xyz123", "default");
        assert_eq!(result, "default");
    }

    #[test]
    fn test_get_env_bool() {
        // Test default
        assert_eq!(get_env_bool("NONEXISTENT_BOOL", false), false);

        // Test parsing
        set_env_var("TEST_BOOL_TRUE", "1");
        assert_eq!(get_env_bool("TEST_BOOL_TRUE", false), true);

        set_env_var("TEST_BOOL_YES", "yes");
        assert_eq!(get_env_bool("TEST_BOOL_YES", false), true);

        set_env_var("TEST_BOOL_FALSE", "0");
        assert_eq!(get_env_bool("TEST_BOOL_FALSE", true), false);

        remove_env_var("TEST_BOOL_TRUE");
        remove_env_var("TEST_BOOL_YES");
        remove_env_var("TEST_BOOL_FALSE");
    }

    #[test]
    fn test_get_env_int() {
        set_env_var("TEST_INT", "42");
        assert_eq!(get_env_int("TEST_INT", 0), 42);
        remove_env_var("TEST_INT");
    }
}
