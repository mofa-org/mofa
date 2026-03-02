//! Security governance core types
//!
//! Defines the fundamental types for PII redaction, content moderation,
//! and security policy configuration used across the MoFA security layer.

use serde::{Deserialize, Serialize};
use std::fmt;
use thiserror::Error;

// =============================================================================
// PII / Sensitive Data Types
// =============================================================================

/// Categories of sensitive data that can be detected and redacted.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[non_exhaustive]
pub enum SensitiveDataCategory {
    /// Email addresses (e.g. user@example.com)
    Email,
    /// Phone numbers (US and international formats)
    Phone,
    /// Credit card numbers (Luhn-validated)
    CreditCard,
    /// US Social Security Numbers
    Ssn,
    /// IPv4 and IPv6 addresses
    IpAddress,
    /// API keys and secrets (common vendor formats)
    ApiKey,
    /// Custom category for domain-specific sensitive data
    Custom(String),
}

impl fmt::Display for SensitiveDataCategory {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Email => write!(f, "email"),
            Self::Phone => write!(f, "phone"),
            Self::CreditCard => write!(f, "credit_card"),
            Self::Ssn => write!(f, "ssn"),
            Self::IpAddress => write!(f, "ip_address"),
            Self::ApiKey => write!(f, "api_key"),
            Self::Custom(name) => write!(f, "custom:{name}"),
        }
    }
}

/// Strategy for redacting detected sensitive data.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub enum RedactionStrategy {
    /// Mask with asterisks, preserving partial structure (e.g. `j***@example.com`)
    Mask,
    /// Replace with a deterministic hash prefix (8 hex chars)
    Hash,
    /// Remove the sensitive text entirely
    Remove,
    /// Replace with a fixed placeholder string
    Replace(String),
}

impl Default for RedactionStrategy {
    fn default() -> Self {
        Self::Mask
    }
}

/// A single match of sensitive data within a text.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RedactionMatch {
    /// Category of the detected sensitive data
    pub category: SensitiveDataCategory,
    /// Byte offset of the match start in the original text
    pub start: usize,
    /// Byte offset of the match end (exclusive) in the original text
    pub end: usize,
    /// The original matched text
    pub original: String,
    /// The replacement text after redaction
    pub replacement: String,
}

/// Result of a redaction operation on a text.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RedactionResult {
    /// The original input text
    pub original_text: String,
    /// The text after redaction
    pub redacted_text: String,
    /// All matches found and redacted
    pub matches: Vec<RedactionMatch>,
}

impl RedactionResult {
    /// Returns `true` if any sensitive data was found and redacted.
    #[must_use]
    pub fn has_redactions(&self) -> bool {
        !self.matches.is_empty()
    }

    /// Returns the number of redacted items.
    #[must_use]
    pub fn redaction_count(&self) -> usize {
        self.matches.len()
    }
}

// =============================================================================
// Content Moderation Types
// =============================================================================

/// Categories of content that can be moderated.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[non_exhaustive]
pub enum ModerationCategory {
    /// Prompt injection attempts (system prompt extraction, role override)
    PromptInjection,
    /// Harmful content (violence, self-harm instructions)
    Harmful,
    /// Toxic or abusive language
    Toxic,
    /// Off-topic or irrelevant content
    OffTopic,
    /// Custom moderation category
    Custom(String),
}

impl fmt::Display for ModerationCategory {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::PromptInjection => write!(f, "prompt_injection"),
            Self::Harmful => write!(f, "harmful"),
            Self::Toxic => write!(f, "toxic"),
            Self::OffTopic => write!(f, "off_topic"),
            Self::Custom(name) => write!(f, "custom:{name}"),
        }
    }
}

/// Verdict from content moderation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub enum ModerationVerdict {
    /// Content is allowed
    Allow,
    /// Content is flagged for review but not blocked
    Flag {
        /// Category that triggered the flag
        category: ModerationCategory,
        /// Human-readable reason
        reason: String,
    },
    /// Content is blocked
    Block {
        /// Category that triggered the block
        category: ModerationCategory,
        /// Human-readable reason
        reason: String,
    },
}

impl ModerationVerdict {
    /// Returns `true` if the verdict allows the content to pass.
    #[must_use]
    pub fn is_allowed(&self) -> bool {
        matches!(self, Self::Allow)
    }

    /// Returns `true` if the content is blocked.
    #[must_use]
    pub fn is_blocked(&self) -> bool {
        matches!(self, Self::Block { .. })
    }
}

/// Policy configuration for content moderation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContentPolicy {
    /// Categories to actively check
    pub enabled_categories: Vec<ModerationCategory>,
    /// Whether to block (true) or just flag (false) on detection
    pub block_on_detection: bool,
}

impl Default for ContentPolicy {
    fn default() -> Self {
        Self {
            enabled_categories: vec![
                ModerationCategory::PromptInjection,
                ModerationCategory::Harmful,
                ModerationCategory::Toxic,
            ],
            block_on_detection: true,
        }
    }
}

// =============================================================================
// Errors
// =============================================================================

/// Errors from the security governance layer.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum SecurityError {
    /// PII detection failed
    #[error("PII detection failed: {0}")]
    DetectionFailed(String),

    /// Redaction operation failed
    #[error("Redaction failed: {0}")]
    RedactionFailed(String),

    /// Content moderation failed
    #[error("Content moderation failed: {0}")]
    ModerationFailed(String),

    /// Security policy violation
    #[error("Policy violation: {category} — {reason}")]
    PolicyViolation {
        /// The violated category
        category: String,
        /// Human-readable reason
        reason: String,
    },

    /// Policy configuration error
    #[error("Invalid security policy: {0}")]
    ConfigurationError(String),
}

/// Result type alias for security operations.
pub type SecurityResult<T> = Result<T, SecurityError>;

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sensitive_data_category_display() {
        assert_eq!(SensitiveDataCategory::Email.to_string(), "email");
        assert_eq!(SensitiveDataCategory::Phone.to_string(), "phone");
        assert_eq!(SensitiveDataCategory::CreditCard.to_string(), "credit_card");
        assert_eq!(SensitiveDataCategory::Ssn.to_string(), "ssn");
        assert_eq!(SensitiveDataCategory::IpAddress.to_string(), "ip_address");
        assert_eq!(SensitiveDataCategory::ApiKey.to_string(), "api_key");
        assert_eq!(
            SensitiveDataCategory::Custom("passport".into()).to_string(),
            "custom:passport"
        );
    }

    #[test]
    fn moderation_category_display() {
        assert_eq!(
            ModerationCategory::PromptInjection.to_string(),
            "prompt_injection"
        );
        assert_eq!(ModerationCategory::Harmful.to_string(), "harmful");
        assert_eq!(ModerationCategory::Toxic.to_string(), "toxic");
        assert_eq!(ModerationCategory::OffTopic.to_string(), "off_topic");
    }

    #[test]
    fn redaction_result_helpers() {
        let empty = RedactionResult {
            original_text: "hello".into(),
            redacted_text: "hello".into(),
            matches: vec![],
        };
        assert!(!empty.has_redactions());
        assert_eq!(empty.redaction_count(), 0);

        let with_match = RedactionResult {
            original_text: "email: test@example.com".into(),
            redacted_text: "email: t***@example.com".into(),
            matches: vec![RedactionMatch {
                category: SensitiveDataCategory::Email,
                start: 7,
                end: 23,
                original: "test@example.com".into(),
                replacement: "t***@example.com".into(),
            }],
        };
        assert!(with_match.has_redactions());
        assert_eq!(with_match.redaction_count(), 1);
    }

    #[test]
    fn moderation_verdict_helpers() {
        assert!(ModerationVerdict::Allow.is_allowed());
        assert!(!ModerationVerdict::Allow.is_blocked());

        let blocked = ModerationVerdict::Block {
            category: ModerationCategory::PromptInjection,
            reason: "system prompt override detected".into(),
        };
        assert!(!blocked.is_allowed());
        assert!(blocked.is_blocked());

        let flagged = ModerationVerdict::Flag {
            category: ModerationCategory::Toxic,
            reason: "potentially toxic language".into(),
        };
        assert!(!flagged.is_allowed());
        assert!(!flagged.is_blocked());
    }

    #[test]
    fn default_content_policy() {
        let policy = ContentPolicy::default();
        assert!(policy.block_on_detection);
        assert_eq!(policy.enabled_categories.len(), 3);
    }

    #[test]
    fn default_redaction_strategy() {
        assert_eq!(RedactionStrategy::default(), RedactionStrategy::Mask);
    }

    #[test]
    fn security_error_display() {
        let err = SecurityError::DetectionFailed("regex error".into());
        assert_eq!(err.to_string(), "PII detection failed: regex error");

        let err = SecurityError::PolicyViolation {
            category: "prompt_injection".into(),
            reason: "blocked".into(),
        };
        assert_eq!(
            err.to_string(),
            "Policy violation: prompt_injection — blocked"
        );
    }

    #[test]
    fn sensitive_data_category_serde_roundtrip() {
        let categories = vec![
            SensitiveDataCategory::Email,
            SensitiveDataCategory::Custom("passport".into()),
        ];
        let json = serde_json::to_string(&categories).unwrap();
        let parsed: Vec<SensitiveDataCategory> = serde_json::from_str(&json).unwrap();
        assert_eq!(categories, parsed);
    }
}
