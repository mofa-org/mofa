//! PII detection and redaction traits
//!
//! Kernel-level contracts for detecting and redacting personally identifiable
//! information (PII) and other sensitive data from text content.

use super::types::{RedactionMatch, RedactionResult, RedactionStrategy, SecurityResult};
use async_trait::async_trait;

// =============================================================================
// PII Detection
// =============================================================================

/// Detects sensitive data patterns in text.
///
/// Implementations scan input text and return all matches of sensitive data
/// categories they are configured to detect.
///
/// # Example
///
/// ```rust,ignore
/// let detector = RegexPiiDetector::default();
/// let matches = detector.detect("Email me at test@example.com").await?;
/// assert_eq!(matches.len(), 1);
/// assert_eq!(matches[0].category, SensitiveDataCategory::Email);
/// ```
#[async_trait]
pub trait PiiDetector: Send + Sync {
    /// Detect all sensitive data matches in the given text.
    async fn detect(&self, text: &str) -> SecurityResult<Vec<RedactionMatch>>;
}

// =============================================================================
// PII Redaction
// =============================================================================

/// Redacts sensitive data from text using a configurable strategy.
///
/// Combines detection and replacement into a single operation,
/// returning the redacted text along with an audit trail of all changes.
#[async_trait]
pub trait PiiRedactor: Send + Sync {
    /// Redact all detected sensitive data from the input text.
    async fn redact(
        &self,
        text: &str,
        strategy: &RedactionStrategy,
    ) -> SecurityResult<RedactionResult>;
}

// =============================================================================
// Audit Logging
// =============================================================================

/// Audit logger for PII redaction events.
///
/// Implementations record redaction operations for compliance and debugging.
/// This is deliberately synchronous since it should not block the hot path.
pub trait RedactionAuditLog: Send + Sync {
    /// Record a redaction event.
    fn log_redaction(&self, result: &RedactionResult);
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::security::types::SensitiveDataCategory;

    // Verify trait object safety — these must compile
    fn _assert_detector_object_safe(_: &dyn PiiDetector) {}
    fn _assert_redactor_object_safe(_: &dyn PiiRedactor) {}
    fn _assert_audit_log_object_safe(_: &dyn RedactionAuditLog) {}

    #[test]
    fn trait_object_safety() {
        // Compile-time check: these trait bounds must be object-safe.
        // If this test compiles, the traits can be used as `dyn Trait`.
        fn _takes_detector(_: Box<dyn PiiDetector>) {}
        fn _takes_redactor(_: Box<dyn PiiRedactor>) {}
        fn _takes_audit(_: Box<dyn RedactionAuditLog>) {}
    }

    #[test]
    fn redaction_result_construction() {
        let result = RedactionResult {
            original_text: "SSN: 123-45-6789".into(),
            redacted_text: "SSN: ***-**-****".into(),
            matches: vec![RedactionMatch {
                category: SensitiveDataCategory::Ssn,
                start: 5,
                end: 16,
                original: "123-45-6789".into(),
                replacement: "***-**-****".into(),
            }],
        };
        assert!(result.has_redactions());
        assert_eq!(result.redaction_count(), 1);
    }
}
