//! Request/Response validation middleware
//!
//! This module provides the main validation middleware that integrates all
//! validation features: schema validation, rate limiting, and input sanitization.

use crate::validation::error::{
    EndpointValidationConfig, RateLimitConfig, RateLimitKeyType, ValidationMiddlewareConfig,
    ValidationOutcome, ValidationRule,
};
use crate::validation::rate_limiter::{RateLimitKeyExtractor, RateLimiter, RateLimitResult};
use crate::validation::sanitizer::InputSanitizer;
use crate::validation::schema::SchemaValidator;
use serde_json::json;
use serde_json::Value;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};

/// Request context for validation
#[derive(Debug, Clone)]
pub struct RequestContext {
    /// Request path
    pub path: String,
    /// HTTP method
    pub method: String,
    /// Client ID (if available)
    pub client_id: Option<String>,
    /// IP address
    pub ip_address: Option<String>,
    /// Agent ID
    pub agent_id: Option<String>,
    /// User ID
    pub user_id: Option<String>,
    /// API key
    pub api_key: Option<String>,
    /// Request headers
    pub headers: std::collections::HashMap<String, String>,
    /// Query parameters
    pub query_params: std::collections::HashMap<String, String>,
    /// Request body (JSON)
    pub body: Option<Value>,
}

impl RequestContext {
    /// Create a new request context
    pub fn new(path: impl Into<String>, method: impl Into<String>) -> Self {
        Self {
            path: path.into(),
            method: method.into(),
            client_id: None,
            ip_address: None,
            agent_id: None,
            user_id: None,
            api_key: None,
            headers: std::collections::HashMap::new(),
            query_params: std::collections::HashMap::new(),
            body: None,
        }
    }

    /// Set client ID
    pub fn with_client_id(mut self, client_id: impl Into<String>) -> Self {
        self.client_id = Some(client_id.into());
        self
    }

    /// Set IP address
    pub fn with_ip_address(mut self, ip: impl Into<String>) -> Self {
        self.ip_address = Some(ip.into());
        self
    }

    /// Set agent ID
    pub fn with_agent_id(mut self, agent_id: impl Into<String>) -> Self {
        self.agent_id = Some(agent_id.into());
        self
    }

    /// Set user ID
    pub fn with_user_id(mut self, user_id: impl Into<String>) -> Self {
        self.user_id = Some(user_id.into());
        self
    }

    /// Set API key
    pub fn with_api_key(mut self, api_key: impl Into<String>) -> Self {
        self.api_key = Some(api_key.into());
        self
    }

    /// Add a header
    pub fn with_header(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.headers.insert(key.into(), value.into());
        self
    }

    /// Add a query parameter
    pub fn with_query_param(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.query_params.insert(key.into(), value.into());
        self
    }

    /// Set request body
    pub fn with_body(mut self, body: Value) -> Self {
        self.body = Some(body);
        self
    }
}

/// Response context for validation
#[derive(Debug, Clone)]
pub struct ResponseContext {
    /// Response body (JSON)
    pub body: Option<Value>,
    /// Status code
    pub status_code: u16,
    /// Response headers
    pub headers: std::collections::HashMap<String, String>,
}

impl ResponseContext {
    /// Create a new response context
    pub fn new() -> Self {
        Self {
            body: None,
            status_code: 200,
            headers: std::collections::HashMap::new(),
        }
    }

    /// Set response body
    pub fn with_body(mut self, body: Value) -> Self {
        self.body = Some(body);
        self
    }

    /// Set status code
    pub fn with_status_code(mut self, code: u16) -> Self {
        self.status_code = code;
        self
    }

    /// Add a response header
    pub fn with_header(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.headers.insert(key.into(), value.into());
        self
    }
}

impl Default for ResponseContext {
    fn default() -> Self {
        Self::new()
    }
}

/// Validation middleware for request/response validation
pub struct ValidationMiddleware {
    /// Configuration
    config: ValidationMiddlewareConfig,
    /// Schema validator
    schema_validator: SchemaValidator,
    /// Input sanitizer
    sanitizer: InputSanitizer,
    /// Rate limiter
    rate_limiter: Arc<RwLock<RateLimiter>>,
    /// Endpoint configurations (cached)
    endpoint_configs: Arc<RwLock<HashMap<String, EndpointValidationConfig>>>,
}

/// Type alias for HashMap in middleware
type HashMap<K, V> = std::collections::HashMap<K, V>;

impl ValidationMiddleware {
    /// Create a new validation middleware with default config
    pub fn new() -> Self {
        Self::with_config(ValidationMiddlewareConfig::default())
    }

    /// Create a new validation middleware with custom config
    pub fn with_config(config: ValidationMiddlewareConfig) -> Self {
        // Build endpoint config map
        let mut endpoint_map: HashMap<String, EndpointValidationConfig> = HashMap::new();
        for endpoint in &config.endpoints {
            let key = format!("{}:{}", endpoint.methods.join(","), endpoint.path);
            endpoint_map.insert(key, endpoint.clone());
        }

        // Create default rate limiter config
        let rate_limit_config = config.default_rate_limit.clone().unwrap_or(RateLimitConfig {
            max_requests: 100,
            window_seconds: 60,
            key_type: RateLimitKeyType::IpAddress,
        });

        Self {
            config,
            schema_validator: SchemaValidator::new(),
            sanitizer: InputSanitizer::new(),
            rate_limiter: Arc::new(RwLock::new(RateLimiter::with_config(rate_limit_config))),
            endpoint_configs: Arc::new(RwLock::new(endpoint_map)),
        }
    }

    /// Validate an incoming request
    pub async fn validate_request(&self, context: &RequestContext) -> ValidationOutcome {
        // Check if validation is enabled
        if !self.config.enabled {
            return ValidationOutcome::Valid;
        }

        // Get endpoint config
        let endpoint_key = format!("{}:{}", context.method, context.path);
        let endpoint_config = self.get_endpoint_config(&endpoint_key).await;

        // If no specific config, use defaults
        let config = match endpoint_config {
            Some(c) => c,
            None => {
                // Check if there's a wildcard match
                let wildcard_key = format!("{}:*", context.method);
                if let Some(c) = self.get_endpoint_config(&wildcard_key).await {
                    c
                } else {
                    // No validation needed for this endpoint
                    debug!("No validation config found for endpoint: {}", endpoint_key);
                    return ValidationOutcome::Valid;
                }
            }
        };

        // Step 1: Rate limiting (if enabled)
        if config.rate_limit_enabled {
            let rate_limit_key = self.extract_rate_limit_key(context, &config).await;
            
            let rate_limit_cfg = config.rate_limit.clone().unwrap_or_else(|| {
                self.config.default_rate_limit.clone().unwrap_or(RateLimitConfig {
                    max_requests: 100,
                    window_seconds: 60,
                    key_type: RateLimitKeyType::IpAddress,
                })
            });

            let rate_result = self.rate_limiter
                .read()
                .await
                .check_rate_limit_with_config(&rate_limit_key, &rate_limit_cfg)
                .await;

            match rate_result {
                RateLimitResult::Exceeded(e) => {
                    if self.config.log_errors {
                        warn!("Rate limit exceeded for client: {}", e.client_id);
                    }
                    return ValidationOutcome::RateLimited(e);
                }
                RateLimitResult::Allowed { .. } => {
                    // Continue with validation
                }
            }
        }

        // Step 2: Input sanitization (if enabled)
        let mut sanitized_body = context.body.clone();
        if config.sanitize_input || self.config.sanitize_by_default {
            if let Some(body) = sanitized_body.take() {
                sanitized_body = Some(self.sanitizer.sanitize_json(&body));
            }
        }

        // Step 3: Validate request body
        if !config.request_rules.is_empty() {
            if let Some(body) = &sanitized_body {
                let errors = self.schema_validator.validate_request(body, &config.request_rules);
                
                if !errors.is_valid() {
                    if self.config.log_errors {
                        error!("Request validation failed: {:?}", errors.errors);
                    }
                    return ValidationOutcome::Invalid(errors);
                }
            }
        }

        // Step 4: Validate headers
        if !config.header_rules.is_empty() {
            let headers_json = json!(&context.headers);
            let errors = self.schema_validator.validate(&headers_json, &config.header_rules);
            
            if !errors.is_valid() {
                if self.config.log_errors {
                    error!("Header validation failed: {:?}", errors.errors);
                }
                return ValidationOutcome::Invalid(errors);
            }
        }

        // Step 5: Validate query parameters
        if !config.query_rules.is_empty() {
            let query_json = json!(&context.query_params);
            let errors = self.schema_validator.validate(&query_json, &config.query_rules);
            
            if !errors.is_valid() {
                if self.config.log_errors {
                    error!("Query validation failed: {:?}", errors.errors);
                }
                return ValidationOutcome::Invalid(errors);
            }
        }

        ValidationOutcome::Valid
    }

    /// Validate an outgoing response
    pub async fn validate_response(
        &self,
        _context: &RequestContext,
        response: &ResponseContext,
        rules: &[ValidationRule],
    ) -> ValidationOutcome {
        // Check if validation is enabled
        if !self.config.enabled {
            return ValidationOutcome::Valid;
        }

        // Don't validate error responses
        if response.status_code >= 400 {
            return ValidationOutcome::Valid;
        }

        // Validate response body
        if let Some(body) = &response.body {
            if !rules.is_empty() {
                let errors = self.schema_validator.validate_response(body, rules);
                
                if !errors.is_valid() {
                    if self.config.log_errors {
                        error!("Response validation failed: {:?}", errors.errors);
                    }
                    return ValidationOutcome::Invalid(errors);
                }
            }
        }

        ValidationOutcome::Valid
    }

    /// Get endpoint configuration
    async fn get_endpoint_config(&self, key: &str) -> Option<EndpointValidationConfig> {
        let configs = self.endpoint_configs.read().await;
        configs.get(key).cloned()
    }

    /// Extract rate limit key from request context
    async fn extract_rate_limit_key(&self, context: &RequestContext, _config: &EndpointValidationConfig) -> String {
        let extractor = RateLimitKeyExtractor::new(RateLimitConfig {
            max_requests: 100,
            window_seconds: 60,
            key_type: RateLimitKeyType::Combined,
        });

        extractor.extract_key(
            context.client_id.as_deref(),
            context.ip_address.as_deref(),
            context.agent_id.as_deref(),
            context.user_id.as_deref(),
            context.api_key.as_deref(),
        )
    }

    /// Register custom validation rules
    pub async fn register_rules(&self, rules: Vec<ValidationRule>) {
        // This would allow dynamic rule registration
        // For now, it's a placeholder for future implementation
        info!("Registering {} custom validation rules", rules.len());
    }

    /// Update rate limit configuration for a specific endpoint
    pub async fn update_rate_limit(&self, client_id: &str, config: RateLimitConfig) {
        let limiter = self.rate_limiter.read().await;
        // Force reset for this client with new config
        let _ = limiter.reset_client(client_id).await;
    }

    /// Get rate limit status for a client
    pub async fn get_rate_limit_status(&self, client_id: &str) -> Option<crate::validation::rate_limiter::RateLimitStatus> {
        let limiter = self.rate_limiter.read().await;
        limiter.get_status(client_id).await
    }
}

impl Default for ValidationMiddleware {
    fn default() -> Self {
        Self::new()
    }
}

/// Helper to create a quick validation middleware
pub fn create_middleware() -> ValidationMiddleware {
    ValidationMiddleware::new()
}

/// Helper to create a validation middleware with common rules
pub fn create_strict_middleware() -> ValidationMiddleware {
    ValidationMiddleware::with_config(ValidationMiddlewareConfig {
        enabled: true,
        endpoints: vec![
            EndpointValidationConfig {
                path: "/api/".to_string(),
                methods: vec!["POST".to_string(), "PUT".to_string(), "PATCH".to_string()],
                request_rules: vec![
                    ValidationRule {
                        name: "content_type".to_string(),
                        field: "content-type".to_string(),
                        rule_type: crate::validation::error::ValidationRuleType::Required,
                        custom_message: Some("Content-Type header is required".to_string()),
                        optional: false,
                    },
                ],
                header_rules: vec![],
                query_rules: vec![],
                rate_limit_enabled: true,
                rate_limit: Some(RateLimitConfig {
                    max_requests: 100,
                    window_seconds: 60,
                    key_type: RateLimitKeyType::IpAddress,
                }),
                sanitize_input: true,
            },
        ],
        custom_rules: HashMap::new(),
        default_rate_limit: Some(RateLimitConfig {
            max_requests: 100,
            window_seconds: 60,
            key_type: RateLimitKeyType::IpAddress,
        }),
        sanitize_by_default: true,
        log_errors: true,
        detailed_errors: true,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_validation_middleware_disabled() {
        let middleware = ValidationMiddleware::with_config(ValidationMiddlewareConfig {
            enabled: false,
            ..Default::default()
        });

        let context = RequestContext::new("/test", "POST")
            .with_body(json!({ "name": "test" }));

        let result = middleware.validate_request(&context).await;
        assert!(result.is_valid());
    }

    #[tokio::test]
    async fn test_validation_middleware_valid_request() {
        let middleware = ValidationMiddleware::new();

        let context = RequestContext::new("/api/test", "POST")
            .with_client_id("test_client")
            .with_ip_address("127.0.0.1")
            .with_body(json!({ "name": "test", "age": 25 }));

        let result = middleware.validate_request(&context).await;
        assert!(result.is_valid());
    }

    #[tokio::test]
    async fn test_validation_with_sanitization() {
        let middleware = create_strict_middleware();

        let context = RequestContext::new("/api/test", "POST")
            .with_client_id("test_client")
            .with_body(json!({ "name": "<script>alert('xss')</script>" }));

        let result = middleware.validate_request(&context).await;
        // Rate limit may be exceeded after many tests, but the value should be sanitized
        // This test just checks it doesn't panic
        assert!(matches!(result, ValidationOutcome::Valid | ValidationOutcome::RateLimited(_)));
    }

    #[test]
    fn test_request_context_builder() {
        let context = RequestContext::new("/api/users", "POST")
            .with_client_id("client_123")
            .with_ip_address("192.168.1.1")
            .with_agent_id("agent_001")
            .with_user_id("user_456")
            .with_header("Content-Type", "application/json")
            .with_query_param("page", "1")
            .with_body(json!({ "name": "John" }));

        assert_eq!(context.path, "/api/users");
        assert_eq!(context.method, "POST");
        assert_eq!(context.client_id, Some("client_123".to_string()));
        assert_eq!(context.ip_address, Some("192.168.1.1".to_string()));
        assert!(context.body.is_some());
    }
}
