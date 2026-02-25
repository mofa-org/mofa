//! Validation middleware error types
//!
//! This module defines the error types used throughout the validation middleware.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Represents the severity level of a validation error
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ValidationSeverity {
    /// Warning - validation failed but request can proceed
    Warning,
    /// Error - validation failed, request should be rejected
    Error,
    /// Critical - validation failed with security implications
    Critical,
}

impl Default for ValidationSeverity {
    fn default() -> Self {
        ValidationSeverity::Error
    }
}

/// Represents a single validation error
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationError {
    /// The field path where validation failed (e.g., "user.name", "messages[0].content")
    pub field: String,
    /// Human-readable error message
    pub message: String,
    /// The value that failed validation (for debugging)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<serde_json::Value>,
    /// Error code for programmatic handling
    pub code: String,
    /// Severity level of the error
    pub severity: ValidationSeverity,
}

impl ValidationError {
    /// Create a new validation error
    pub fn new(field: impl Into<String>, message: impl Into<String>, code: impl Into<String>) -> Self {
        Self {
            field: field.into(),
            message: message.into(),
            value: None,
            code: code.into(),
            severity: ValidationSeverity::Error,
        }
    }

    /// Create a validation error with a specific value
    pub fn with_value(
        field: impl Into<String>,
        message: impl Into<String>,
        code: impl Into<String>,
        value: serde_json::Value,
    ) -> Self {
        Self {
            field: field.into(),
            message: message.into(),
            value: Some(value),
            code: code.into(),
            severity: ValidationSeverity::Error,
        }
    }

    /// Create a validation error with a specific severity
    pub fn with_severity(
        field: impl Into<String>,
        message: impl Into<String>,
        code: impl Into<String>,
        severity: ValidationSeverity,
    ) -> Self {
        Self {
            field: field.into(),
            message: message.into(),
            value: None,
            code: code.into(),
            severity,
        }
    }
}

/// Result type for validation operations
pub type ValidationResult<T> = Result<T, ValidationError>;

/// Collection of validation errors with metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationErrorCollection {
    /// List of individual validation errors
    pub errors: Vec<ValidationError>,
    /// Summary message
    pub summary: String,
    /// Whether validation passed
    #[serde(skip)]
    pub is_valid: bool,
}

impl ValidationErrorCollection {
    /// Create a new empty validation error collection
    pub fn new() -> Self {
        Self {
            errors: Vec::new(),
            summary: String::new(),
            is_valid: true,
        }
    }

    /// Create a validation error collection with initial errors
    pub fn with_errors(errors: Vec<ValidationError>) -> Self {
        let is_valid = errors.is_empty();
        let summary = if is_valid {
            "Validation passed".to_string()
        } else {
            format!("Validation failed with {} error(s)", errors.len())
        };
        
        Self {
            errors,
            summary,
            is_valid,
        }
    }

    /// Add an error to the collection
    pub fn add_error(&mut self, error: ValidationError) {
        self.is_valid = false;
        self.errors.push(error);
        self.summary = format!("Validation failed with {} error(s)", self.errors.len());
    }

    /// Check if validation passed
    pub fn is_valid(&self) -> bool {
        self.is_valid
    }

    /// Get the number of errors
    pub fn error_count(&self) -> usize {
        self.errors.len()
    }

    /// Get errors filtered by severity
    pub fn errors_by_severity(&self, severity: ValidationSeverity) -> Vec<&ValidationError> {
        self.errors.iter().filter(|e| e.severity == severity).collect()
    }

    /// Check if there are any critical errors
    pub fn has_critical_errors(&self) -> bool {
        self.errors.iter().any(|e| e.severity == ValidationSeverity::Critical)
    }
}

impl Default for ValidationErrorCollection {
    fn default() -> Self {
        Self::new()
    }
}

/// Rate limit error details
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RateLimitError {
    /// Client identifier
    pub client_id: String,
    /// Maximum requests allowed
    pub max_requests: u32,
    /// Current request count
    pub current_count: u32,
    /// Window duration in seconds
    pub window_seconds: u64,
    /// When the rate limit resets
    pub resets_at: chrono::DateTime<chrono::Utc>,
}

impl RateLimitError {
    pub fn new(
        client_id: String,
        max_requests: u32,
        current_count: u32,
        window_seconds: u64,
        resets_at: chrono::DateTime<chrono::Utc>,
    ) -> Self {
        Self {
            client_id,
            max_requests,
            current_count,
            window_seconds,
            resets_at,
        }
    }
}

/// Comprehensive validation result that includes both validation errors and rate limiting
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ValidationOutcome {
    /// Validation passed
    Valid,
    /// Validation failed with errors
    Invalid(ValidationErrorCollection),
    /// Rate limit exceeded
    RateLimited(RateLimitError),
}

impl ValidationOutcome {
    /// Check if the validation passed
    pub fn is_valid(&self) -> bool {
        matches!(self, ValidationOutcome::Valid)
    }

    /// Get validation errors if any
    pub fn errors(&self) -> Option<&ValidationErrorCollection> {
        match self {
            ValidationOutcome::Invalid(errors) => Some(errors),
            _ => None,
        }
    }

    /// Get rate limit error if any
    pub fn rate_limit_error(&self) -> Option<&RateLimitError> {
        match self {
            ValidationOutcome::RateLimited(error) => Some(error),
            _ => None,
        }
    }
}

/// Validation rule configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationRule {
    /// Rule name identifier
    pub name: String,
    /// Field path to apply the rule to
    pub field: String,
    /// Rule type (required, min_length, max_length, pattern, etc.)
    pub rule_type: ValidationRuleType,
    /// Custom error message
    #[serde(skip_serializing_if = "Option::is_none")]
    pub custom_message: Option<String>,
    /// Whether to skip validation if field is missing
    pub optional: bool,
}

/// Types of validation rules
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "params")]
pub enum ValidationRuleType {
    /// Field must be present and not null
    Required,
    /// String minimum length
    MinLength(usize),
    /// String maximum length
    MaxLength(usize),
    /// Number minimum value
    MinValue(f64),
    /// Number maximum value
    MaxValue(f64),
    /// Regex pattern match
    Pattern(String),
    /// Enum allowed values
    AllowedValues(Vec<serde_json::Value>),
    /// Email format validation
    Email,
    /// URL format validation
    Url,
    /// UUID format validation
    Uuid,
    /// JSON schema validation
    Schema(serde_json::Value),
    /// Custom validator function name (for runtime validation)
    Custom(String),
}

/// Endpoint-specific validation configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EndpointValidationConfig {
    /// Endpoint path (e.g., "/api/agents", "/api/messages")
    pub path: String,
    /// HTTP methods this config applies to
    pub methods: Vec<String>,
    /// Validation rules for request body
    #[serde(default)]
    pub request_rules: Vec<ValidationRule>,
    /// Validation rules for request headers
    #[serde(default)]
    pub header_rules: Vec<ValidationRule>,
    /// Validation rules for query parameters
    #[serde(default)]
    pub query_rules: Vec<ValidationRule>,
    /// Whether to enable rate limiting for this endpoint
    pub rate_limit_enabled: bool,
    /// Rate limit configuration (requests per window)
    pub rate_limit: Option<RateLimitConfig>,
    /// Whether to sanitize input
    pub sanitize_input: bool,
}

/// Rate limiting configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RateLimitConfig {
    /// Maximum requests allowed in the window
    pub max_requests: u32,
    /// Time window in seconds
    pub window_seconds: u64,
    /// Key generator function (client_id, ip, agent_id, etc.)
    pub key_type: RateLimitKeyType,
}

/// Types of keys for rate limiting
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RateLimitKeyType {
    /// Rate limit by client ID
    ClientId,
    /// Rate limit by IP address
    IpAddress,
    /// Rate limit by agent ID
    AgentId,
    /// Rate limit by user ID
    UserId,
    /// Rate limit by API key
    ApiKey,
    /// Combined rate limit (IP + Client ID)
    Combined,
}

impl Default for RateLimitKeyType {
    fn default() -> Self {
        RateLimitKeyType::IpAddress
    }
}

/// Validation configuration for the middleware
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationMiddlewareConfig {
    /// Global validation enabled
    pub enabled: bool,
    /// Default endpoint configurations
    #[serde(default)]
    pub endpoints: Vec<EndpointValidationConfig>,
    /// Custom validation rules
    #[serde(default)]
    pub custom_rules: HashMap<String, serde_json::Value>,
    /// Default rate limit config (used if endpoint doesn't specify)
    pub default_rate_limit: Option<RateLimitConfig>,
    /// Enable input sanitization by default
    pub sanitize_by_default: bool,
    /// Log validation errors
    pub log_errors: bool,
    /// Return detailed error messages (include field paths, values, etc.)
    pub detailed_errors: bool,
}

impl Default for ValidationMiddlewareConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            endpoints: Vec::new(),
            custom_rules: HashMap::new(),
            default_rate_limit: Some(RateLimitConfig {
                max_requests: 100,
                window_seconds: 60,
                key_type: RateLimitKeyType::IpAddress,
            }),
            sanitize_by_default: true,
            log_errors: true,
            detailed_errors: true,
        }
    }
}
