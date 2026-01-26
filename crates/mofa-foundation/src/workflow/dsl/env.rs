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
/// std::env::set_var("API_HOST", "example.com");
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
        serde_json::Value::String(s) => {
            serde_json::Value::String(substitute_env(s))
        }
        serde_json::Value::Array(arr) => {
            arr.iter()
                .map(substitute_env_recursive)
                .collect()
        }
        serde_json::Value::Object(obj) => {
            obj.iter()
                .map(|(k, v)| (k.clone(), substitute_env_recursive(v)))
                .collect()
        }
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
            vars.get(var_name)
                .map(|v| v.as_str())
                .unwrap_or_else(|| {
                    std::env::var(var_name)
                        .unwrap_or_else(|_| caps[0].to_string())
                        .leak()
                })
        })
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_substitute_env_single() {
        std::env::set_var("TEST_VAR", "hello");
        let result = substitute_env("prefix_${TEST_VAR}_suffix");
        assert_eq!(result, "prefix_hello_suffix");
    }

    #[test]
    fn test_substitute_env_multiple() {
        std::env::set_var("VAR1", "foo");
        std::env::set_var("VAR2", "bar");
        let result = substitute_env("${VAR1}_${VAR2}");
        assert_eq!(result, "foo_bar");
    }

    #[test]
    fn test_substitute_env_missing() {
        let result = substitute_env("prefix_${MISSING_VAR}_suffix");
        assert_eq!(result, "prefix_${MISSING_VAR}_suffix");
    }

    #[test]
    fn test_substitute_env_recursive() {
        std::env::set_var("API_KEY", "secret123");
        let value = json!({
            "url": "https://${API_HOST}/api",
            "key": "${API_KEY}",
            "nested": {
                "value": "${VAR}"
            }
        });
        std::env::set_var("API_HOST", "api.example.com");
        std::env::set_var("VAR", "test");

        let result = substitute_env_recursive(&value);
        assert_eq!(
            result["url"],
            "https://api.example.com/api"
        );
        assert_eq!(result["key"], "secret123");
        assert_eq!(result["nested"]["value"], "test");
    }

    #[test]
    fn test_substitute_with_custom_vars() {
        let vars = HashMap::from([
            ("VAR1".to_string(), "custom".to_string()),
            ("VAR2".to_string(), "value".to_string()),
        ]);
        let result = substitute_with("${VAR1}_${VAR2}", &vars);
        assert_eq!(result, "custom_value");
    }
}
