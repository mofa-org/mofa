//! Security Types
//!
//! Core data types for security governance, including:
//! - Sensitive data categories for PII detection
//! - Redaction strategies for data sanitization
//! - Moderation verdicts for content filtering

use serde::{Deserialize, Serialize};

/// Categories of sensitive data that can be detected and redacted
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[non_exhaustive]
pub enum SensitiveDataCategory {
    /// Email addresses
    Email,
    /// Phone numbers
    Phone,
    /// Credit card numbers
    CreditCard,
    /// Social Security Numbers (US)
    SSN,
    /// IP addresses
    IpAddress,
    /// API keys and tokens
    ApiKey,
    /// Custom category (user-defined)
    Custom(String),
}

impl SensitiveDataCategory {
    /// Get a human-readable name for the category
    pub fn name(&self) -> &str {
        match self {
            SensitiveDataCategory::Email => "email",
            SensitiveDataCategory::Phone => "phone",
            SensitiveDataCategory::CreditCard => "credit_card",
            SensitiveDataCategory::SSN => "ssn",
            SensitiveDataCategory::IpAddress => "ip_address",
            SensitiveDataCategory::ApiKey => "api_key",
            SensitiveDataCategory::Custom(s) => s,
        }
    }
}

/// Strategy for redacting sensitive data
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub enum RedactionStrategy {
    /// Mask with [REDACTED] placeholder
    Mask,
    /// Replace with SHA-256 hash
    Hash,
    /// Remove entirely
    Remove,
    /// Replace with custom string (stored separately)
    Replace,
}

/// Moderation verdict for content filtering
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub enum ModerationVerdict {
    /// Content is allowed
    Allow,
    /// Content is flagged but allowed (with reason)
    Flag(String),
    /// Content is blocked (with reason)
    Block(String),
}

impl ModerationVerdict {
    /// Check if content is allowed
    pub fn is_allowed(&self) -> bool {
        matches!(self, ModerationVerdict::Allow | ModerationVerdict::Flag(_))
    }

    /// Check if content is blocked
    pub fn is_blocked(&self) -> bool {
        matches!(self, ModerationVerdict::Block(_))
    }

    /// Get the reason if flagged or blocked
    pub fn reason(&self) -> Option<&str> {
        match self {
            ModerationVerdict::Allow => None,
            ModerationVerdict::Flag(reason) | ModerationVerdict::Block(reason) => Some(reason),
        }
    }
}

/// Detected PII information
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DetectedPii {
    /// Category of sensitive data
    pub category: SensitiveDataCategory,
    /// The detected value (may be partial/masked)
    pub value: String,
    /// Start position in the text
    pub start: usize,
    /// End position in the text
    pub end: usize,
}

/// Security fail mode: how to handle security check failures
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[non_exhaustive]
pub enum SecurityFailMode {
    /// Fail open: allow on error (more permissive, better UX)
    FailOpen,
    /// Fail closed: deny on error (more secure, stricter)
    #[default]
    FailClosed,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sensitive_data_category_name() {
        assert_eq!(SensitiveDataCategory::Email.name(), "email");
        assert_eq!(SensitiveDataCategory::Custom("custom".to_string()).name(), "custom");
    }

    #[test]
    fn test_moderation_verdict() {
        assert!(ModerationVerdict::Allow.is_allowed());
        assert!(!ModerationVerdict::Allow.is_blocked());
        
        assert!(ModerationVerdict::Flag("test".to_string()).is_allowed());
        assert!(!ModerationVerdict::Flag("test".to_string()).is_blocked());
        
        assert!(!ModerationVerdict::Block("test".to_string()).is_allowed());
        assert!(ModerationVerdict::Block("test".to_string()).is_blocked());
        
        assert_eq!(ModerationVerdict::Allow.reason(), None);
        assert_eq!(ModerationVerdict::Flag("reason".to_string()).reason(), Some("reason"));
    }
}
