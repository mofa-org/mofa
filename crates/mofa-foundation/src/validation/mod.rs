//! Validation Middleware for MoFA
//!
//! This module provides comprehensive request/response validation middleware
//! with the following features:
//!
//! - **Schema-based request validation**: Validate JSON requests against defined schemas
//! - **Response validation**: Ensure response consistency
//! - **Rate limiting**: Per-client/agent rate limiting to prevent abuse
//! - **Input sanitization**: Security-focused input sanitization
//! - **Custom validation rules**: Define per-endpoint validation rules
//! - **Detailed error reporting**: Comprehensive validation error messages
//!
//! # Example
//!
//! ```rust
//! use mofa_foundation::validation::{
//!     ValidationMiddleware, RequestContext, ResponseContext,
//!     create_middleware, create_strict_middleware,
//! };
//! use serde_json::json;
//!
//! #[tokio::main]
//! async fn main() {
//!     let middleware = create_middleware();
//!
//!     // Validate a request
//!     let request = RequestContext::new("/api/users", "POST")
//!         .with_client_id("client_123")
//!         .with_body(json!({ "name": "John", "email": "john@example.com" }));
//!
//!     let result = middleware.validate_request(&request).await;
//!     println!("Validation result: {:?}", result.is_valid());
//! }
//! ```
//!
//! # Feature Overview
//!
//! ## Schema Validation
//!
//! The schema validator supports various validation rules:
//! - Required fields
//! - String length (min/max)
//! - Numeric ranges (min/max values)
//! - Pattern matching (regex)
//! - Email, URL, UUID format validation
//! - JSON Schema validation
//!
//! ## Rate Limiting
//!
//! Rate limiting can be configured per-endpoint with different key types:
//! - Client ID
//! - IP Address
//! - Agent ID
//! - User ID
//! - API Key
//! - Combined (IP + Client ID)
//!
//! ## Input Sanitization
//!
//! The sanitizer provides protection against:
//! - XSS attacks (script tag removal, HTML escaping)
//! - SQL injection (pattern removal)
//! - Path traversal
//! - Control character stripping

pub mod error;
pub mod middleware;
pub mod rate_limiter;
pub mod sanitizer;
pub mod schema;

// Re-export main types
pub use error::{
    EndpointValidationConfig, RateLimitConfig, RateLimitError, RateLimitKeyType,
    ValidationError, ValidationErrorCollection, ValidationMiddlewareConfig, ValidationOutcome,
    ValidationResult, ValidationRule, ValidationRuleType, ValidationSeverity,
};

pub use middleware::{
    create_middleware, create_strict_middleware, RequestContext, ResponseContext,
    ValidationMiddleware,
};

pub use rate_limiter::{RateLimitResult, RateLimitStatus, RateLimitKeyExtractor, RateLimiter};

pub use sanitizer::{InputSanitizer, SanitizerConfig};

pub use schema::SchemaValidator;
