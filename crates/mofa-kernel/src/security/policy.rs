//! Security policy builder
//!
//! Provides a fluent API for constructing security policies that combine
//! PII redaction rules, content moderation settings, and audit configuration.

use super::types::{
    ContentPolicy, ModerationCategory, RedactionStrategy, SecurityError, SecurityResult,
    SensitiveDataCategory,
};
use serde::{Deserialize, Serialize};

// =============================================================================
// Security Policy
// =============================================================================

/// A complete security policy combining PII and moderation rules.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityPolicy {
    /// PII categories to detect and redact
    pub pii_categories: Vec<SensitiveDataCategory>,
    /// Strategy for redacting detected PII
    pub redaction_strategy: RedactionStrategy,
    /// Content moderation policy
    pub content_policy: ContentPolicy,
    /// Whether to enable audit logging for redaction events
    pub audit_enabled: bool,
}

impl Default for SecurityPolicy {
    fn default() -> Self {
        Self {
            pii_categories: vec![
                SensitiveDataCategory::Email,
                SensitiveDataCategory::Phone,
                SensitiveDataCategory::CreditCard,
                SensitiveDataCategory::Ssn,
            ],
            redaction_strategy: RedactionStrategy::Mask,
            content_policy: ContentPolicy::default(),
            audit_enabled: false,
        }
    }
}

// =============================================================================
// Policy Builder
// =============================================================================

/// Builder for constructing `SecurityPolicy` instances.
///
/// # Example
///
/// ```rust,ignore
/// let policy = PolicyBuilder::new()
///     .with_pii_categories(vec![SensitiveDataCategory::Email, SensitiveDataCategory::Phone])
///     .with_redaction_strategy(RedactionStrategy::Hash)
///     .with_moderation_categories(vec![ModerationCategory::PromptInjection])
///     .block_on_detection(true)
///     .with_audit(true)
///     .build()?;
/// ```
#[derive(Debug, Default)]
pub struct PolicyBuilder {
    pii_categories: Option<Vec<SensitiveDataCategory>>,
    redaction_strategy: Option<RedactionStrategy>,
    moderation_categories: Option<Vec<ModerationCategory>>,
    block_on_detection: Option<bool>,
    audit_enabled: Option<bool>,
}

impl PolicyBuilder {
    /// Create a new policy builder with default settings.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the PII categories to detect.
    #[must_use]
    pub fn with_pii_categories(mut self, categories: Vec<SensitiveDataCategory>) -> Self {
        self.pii_categories = Some(categories);
        self
    }

    /// Set the redaction strategy.
    #[must_use]
    pub fn with_redaction_strategy(mut self, strategy: RedactionStrategy) -> Self {
        self.redaction_strategy = Some(strategy);
        self
    }

    /// Set the moderation categories to check.
    #[must_use]
    pub fn with_moderation_categories(mut self, categories: Vec<ModerationCategory>) -> Self {
        self.moderation_categories = Some(categories);
        self
    }

    /// Set whether to block on detection (true) or just flag (false).
    #[must_use]
    pub fn block_on_detection(mut self, block: bool) -> Self {
        self.block_on_detection = Some(block);
        self
    }

    /// Enable or disable audit logging.
    #[must_use]
    pub fn with_audit(mut self, enabled: bool) -> Self {
        self.audit_enabled = Some(enabled);
        self
    }

    /// Build the security policy.
    ///
    /// Returns an error if the configuration is invalid (e.g. no categories).
    pub fn build(self) -> SecurityResult<SecurityPolicy> {
        let defaults = SecurityPolicy::default();

        let pii_categories = self.pii_categories.unwrap_or(defaults.pii_categories);
        let moderation_categories = self
            .moderation_categories
            .unwrap_or(defaults.content_policy.enabled_categories);

        if pii_categories.is_empty() && moderation_categories.is_empty() {
            return Err(SecurityError::ConfigurationError(
                "At least one PII category or moderation category must be enabled".into(),
            ));
        }

        Ok(SecurityPolicy {
            pii_categories,
            redaction_strategy: self.redaction_strategy.unwrap_or(defaults.redaction_strategy),
            content_policy: ContentPolicy {
                enabled_categories: moderation_categories,
                block_on_detection: self
                    .block_on_detection
                    .unwrap_or(defaults.content_policy.block_on_detection),
            },
            audit_enabled: self.audit_enabled.unwrap_or(defaults.audit_enabled),
        })
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_policy_has_sensible_defaults() {
        let policy = SecurityPolicy::default();
        assert_eq!(policy.pii_categories.len(), 4);
        assert_eq!(policy.redaction_strategy, RedactionStrategy::Mask);
        assert!(policy.content_policy.block_on_detection);
        assert!(!policy.audit_enabled);
    }

    #[test]
    fn builder_with_defaults() {
        let policy = PolicyBuilder::new().build().unwrap();
        assert_eq!(policy.pii_categories.len(), 4);
        assert_eq!(policy.content_policy.enabled_categories.len(), 3);
    }

    #[test]
    fn builder_custom_pii_categories() {
        let policy = PolicyBuilder::new()
            .with_pii_categories(vec![SensitiveDataCategory::Email])
            .build()
            .unwrap();
        assert_eq!(policy.pii_categories.len(), 1);
        assert_eq!(policy.pii_categories[0], SensitiveDataCategory::Email);
    }

    #[test]
    fn builder_custom_redaction_strategy() {
        let policy = PolicyBuilder::new()
            .with_redaction_strategy(RedactionStrategy::Hash)
            .build()
            .unwrap();
        assert_eq!(policy.redaction_strategy, RedactionStrategy::Hash);
    }

    #[test]
    fn builder_custom_moderation() {
        let policy = PolicyBuilder::new()
            .with_moderation_categories(vec![ModerationCategory::PromptInjection])
            .block_on_detection(false)
            .build()
            .unwrap();
        assert_eq!(policy.content_policy.enabled_categories.len(), 1);
        assert!(!policy.content_policy.block_on_detection);
    }

    #[test]
    fn builder_with_audit() {
        let policy = PolicyBuilder::new().with_audit(true).build().unwrap();
        assert!(policy.audit_enabled);
    }

    #[test]
    fn builder_rejects_empty_categories() {
        let result = PolicyBuilder::new()
            .with_pii_categories(vec![])
            .with_moderation_categories(vec![])
            .build();
        assert!(result.is_err());
    }

    #[test]
    fn policy_serde_roundtrip() {
        let policy = PolicyBuilder::new()
            .with_pii_categories(vec![
                SensitiveDataCategory::Email,
                SensitiveDataCategory::CreditCard,
            ])
            .with_redaction_strategy(RedactionStrategy::Replace("[REDACTED]".into()))
            .with_audit(true)
            .build()
            .unwrap();

        let json = serde_json::to_string(&policy).unwrap();
        let parsed: SecurityPolicy = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.pii_categories.len(), 2);
        assert!(parsed.audit_enabled);
    }
}
