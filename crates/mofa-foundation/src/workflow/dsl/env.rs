//! Environment variable substitution for DSL configs
//!
//! Supports ${VAR_NAME} syntax in workflow definitions.

use once_cell::sync::Lazy;
use regex::Regex;
use std::collections::HashMap;

/// Regex for matching ${VAR_NAME} patterns
static ENV_VAR_REGEX: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"\$\{([A-Za-z_][A-Za-z0-9_]*)\}").unwrap());

/// Substitute environment variables in a string
///
/// Replaces ${VAR_NAME} patterns with values from environment variables.
/// If a variable is not set, the pattern is left unchanged.
///
/// # Example
///
/// ```rust
/// use mofa_foundation::workflow::dsl::env::substitute_env;
///
/// let input = "https://${API_HOST}/api";
/// unsafe { std::env::set_var("API_HOST", "example.com"); }
/// let output = substitute_env(input);
/// assert_eq!(output, "https://example.com/api");
/// ```
pub fn substitute_env(input: &str) -> String {
    ENV_VAR_REGEX
        .replace_all(input, |caps: &regex::Captures| {
            let var_name = &caps[1];
            std::env::var(var_name).unwrap_or_else(|_| caps[0].to_string())
        })
        .to_string()
}

/// Substitute environment variables in all string values of a YAML/JSON structure
pub fn substitute_env_recursive(value: &serde_json::Value) -> serde_json::Value {
    match value {
        serde_json::Value::String(s) => serde_json::Value::String(substitute_env(s)),
        serde_json::Value::Array(arr) => arr.iter().map(substitute_env_recursive).collect(),
        serde_json::Value::Object(obj) => obj
            .iter()
            .map(|(k, v)| (k.clone(), substitute_env_recursive(v)))
            .collect(),
        _ => value.clone(),
    }
}

/// Substitute environment variables with a custom mapping
///
/// Useful for testing or when you want to provide explicit variable values.
pub fn substitute_with(input: &str, vars: &HashMap<String, String>) -> String {
    ENV_VAR_REGEX
        .replace_all(input, |caps: &regex::Captures| {
            let var_name = &caps[1];
            vars.get(var_name).map(|v| v.as_str()).unwrap_or_else(|| {
                std::env::var(var_name)
                    .unwrap_or_else(|_| caps[0].to_string())
                    .leak()
            })
        })
        .to_string()
}
