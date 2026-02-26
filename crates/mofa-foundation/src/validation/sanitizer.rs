//! Input sanitization for security
//!
//! This module provides functions to sanitize user input and prevent common security issues
//! such as XSS attacks, SQL injection, and other injection-based vulnerabilities.

use html_escape;
use regex::Regex;
use serde_json::{Map, Value};

/// Sanitizer configuration
#[derive(Debug, Clone)]
pub struct SanitizerConfig {
    /// Enable HTML escaping
    pub escape_html: bool,
    /// Enable JavaScript removal
    pub remove_scripts: bool,
    /// Enable SQL injection prevention
    pub prevent_sql_injection: bool,
    /// Enable URL sanitization
    pub sanitize_urls: bool,
    /// Maximum string length
    pub max_length: Option<usize>,
    /// Strip control characters
    pub strip_control_chars: bool,
}

impl Default for SanitizerConfig {
    fn default() -> Self {
        Self {
            escape_html: true,
            remove_scripts: true,
            prevent_sql_injection: true,
            sanitize_urls: true,
            max_length: Some(100_000), // 100KB default
            strip_control_chars: true,
        }
    }
}

/// Input sanitizer for security
pub struct InputSanitizer {
    config: SanitizerConfig,
    // Precompiled patterns
    script_pattern: Regex,
    sql_pattern: Regex,
    control_char_pattern: Regex,
}

impl InputSanitizer {
    /// Create a new input sanitizer with default config
    pub fn new() -> Self {
        Self::with_config(SanitizerConfig::default())
    }

    /// Create a new input sanitizer with custom config
    pub fn with_config(config: SanitizerConfig) -> Self {
        let script_pattern = Regex::new(r"<script[^>]*>.*?</script>").unwrap();
        let sql_pattern = Regex::new(
            r"(?i)(union\s+select|insert\s+into|delete\s+from|drop\s+table|update\s+\w+\s+set|--|\#|\/\*|\*\/)"
        ).unwrap();
        let control_char_pattern = Regex::new(r"[\x00-\x08\x0B\x0C\x0E-\x1F\x7F]").unwrap();

        Self {
            config,
            script_pattern,
            sql_pattern,
            control_char_pattern,
        }
    }

    /// Sanitize a string value
    pub fn sanitize_string(&self, input: &str) -> String {
        let mut result = input.to_string();

        // Strip control characters
        if self.config.strip_control_chars {
            result = self.control_char_pattern.replace_all(&result, "").to_string();
        }

        // Remove script tags
        if self.config.remove_scripts {
            result = self.script_pattern.replace_all(&result, "").to_string();
            // Also remove javascript: URLs
            result = result.replace("javascript:", "");
            // Remove event handlers
            result = Regex::new(r"\s*on\w+\s*=").unwrap()
                .replace_all(&result, "")
                .to_string();
        }

        // Escape HTML
        if self.config.escape_html {
            result = html_escape::encode_text(&result).to_string();
        }

        // Prevent SQL injection (escape special characters)
        if self.config.prevent_sql_injection {
            result = self.sql_pattern.replace_all(&result, "").to_string();
        }

        // Truncate if too long
        if let Some(max_len) = self.config.max_length {
            if result.len() > max_len {
                result.truncate(max_len);
            }
        }

        result
    }

    /// Sanitize a JSON value recursively
    pub fn sanitize_json(&self, value: &Value) -> Value {
        match value {
            Value::String(s) => Value::String(self.sanitize_string(s)),
            Value::Array(arr) => {
                Value::Array(arr.iter().map(|v| self.sanitize_json(v)).collect())
            }
            Value::Object(obj) => {
                let mut new_obj = Map::new();
                for (k, v) in obj {
                    new_obj.insert(k.clone(), self.sanitize_json(v));
                }
                Value::Object(new_obj)
            }
            other => other.clone(),
        }
    }

    /// Sanitize a URL
    pub fn sanitize_url(&self, url: &str) -> Option<String> {
        // Only allow http and https
        if !url.starts_with("http://") && !url.starts_with("https://") {
            return None;
        }

        // Parse and validate URL
        if let Ok(parsed) = url::Url::parse(url) {
            // Check for dangerous schemes - scheme() returns &str directly
            let scheme = parsed.scheme();
            match scheme {
                "http" | "https" => Some(url.to_string()),
                _ => None,
            }
        } else {
            None
        }
    }

    /// Check if a string contains potentially dangerous content
    pub fn contains_dangerous_content(&self, input: &str) -> bool {
        // Check for script tags
        if self.script_pattern.is_match(input) {
            return true;
        }

        // Check for javascript: URLs
        if input.contains("javascript:") {
            return true;
        }

        // Check for event handlers
        if Regex::new(r"\s*on\w+\s*=\s*").unwrap().is_match(input) {
            return true;
        }

        false
    }

    /// Validate and sanitize an API key
    pub fn sanitize_api_key(&self, key: &str) -> Option<String> {
        // API keys should only contain alphanumeric characters and some special chars
        let valid_pattern = Regex::new(r"^[a-zA-Z0-9_\-\.]+$").unwrap();
        
        if valid_pattern.is_match(key) {
            // Mask the key for logging
            if key.len() > 8 {
                Some(format!("{}...{}", &key[..4], &key[key.len()-4..]))
            } else {
                Some("****".to_string())
            }
        } else {
            None
        }
    }

    /// Sanitize a file path to prevent path traversal
    pub fn sanitize_path(&self, path: &str) -> String {
        // Use a placeholder approach to properly handle path traversal
        let placeholder = "\x00"; // Unlikely character
        let mut result = path.to_string();
        
        // First replace "../" with placeholder to avoid consuming the following slash
        // Keep doing this until no more "../" sequences are found
        loop {
            let new_result = result.replace("../", placeholder);
            if new_result == result {
                break;
            }
            result = new_result;
        }
        
        // Now replace remaining ".." (those without following slash)
        result = result.replace("..", "");
        
        // Replace placeholder with slash
        result = result.replace(placeholder, "/");
        
        // Clean up multiple slashes
        while result.contains("//") {
            result = result.replace("//", "/");
        }
        
        // Remove null bytes
        result = result.replace("\0", "");
        
        // Ensure path starts with / for absolute paths
        if path.starts_with('/') && !result.starts_with('/') && !result.is_empty() {
            result = format!("/{}", result);
        }
        
        result
    }

    /// Sanitize email address
    pub fn sanitize_email(&self, email: &str) -> Option<String> {
        let email_pattern = Regex::new(r"^[a-zA-Z0-9._%+-]+@[a-zA-Z0-9.-]+\.[a-zA-Z]{2,}$").unwrap();
        
        if email_pattern.is_match(email) {
            Some(email.to_lowercase())
        } else {
            None
        }
    }
}

impl Default for InputSanitizer {
    fn default() -> Self {
        Self::new()
    }
}

/// Sanitize request headers
pub fn sanitize_headers(headers: &mut std::collections::HashMap<String, String>) {
    let sanitizer = InputSanitizer::new();
    
    for value in headers.values_mut() {
        *value = sanitizer.sanitize_string(value);
    }
}

/// Sanitize query parameters
pub fn sanitize_query_params(params: &mut std::collections::HashMap<String, String>) {
    let sanitizer = InputSanitizer::new();
    
    for value in params.values_mut() {
        *value = sanitizer.sanitize_string(value);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_sanitize_html() {
        let sanitizer = InputSanitizer::new();
        let input = "<script>alert('xss')</script>Hello";
        let result = sanitizer.sanitize_string(input);
        assert!(!result.contains("<script>"));
    }

    #[test]
    fn test_sanitize_sql() {
        let sanitizer = InputSanitizer::new();
        let input = "'; DROP TABLE users; --";
        let result = sanitizer.sanitize_string(input);
        assert!(!result.contains("DROP TABLE"));
    }

    #[test]
    fn test_sanitize_json() {
        let sanitizer = InputSanitizer::new();
        let input = json!({
            "name": "<script>alert('xss')</script>",
            "bio": "Normal text"
        });
        let result = sanitizer.sanitize_json(&input);
        
        if let Some(name) = result.get("name").and_then(|v| v.as_str()) {
            assert!(!name.contains("<script>"));
        }
    }

    #[test]
    fn test_dangerous_content_detection() {
        let sanitizer = InputSanitizer::new();
        
        assert!(sanitizer.contains_dangerous_content("<script>alert(1)</script>"));
        assert!(sanitizer.contains_dangerous_content("javascript:alert(1)"));
        assert!(sanitizer.contains_dangerous_content("<img onerror='alert(1)'>"));
        assert!(!sanitizer.contains_dangerous_content("Hello World"));
    }

    #[test]
    fn test_path_sanitization() {
        let sanitizer = InputSanitizer::new();
        
        assert_eq!(sanitizer.sanitize_path("/etc/passwd"), "/etc/passwd");
        assert_eq!(sanitizer.sanitize_path("../../../etc/passwd"), "/etc/passwd");
    }
}
