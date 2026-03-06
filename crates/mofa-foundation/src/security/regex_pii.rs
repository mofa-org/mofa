//! Regex-based PII detection and redaction
//!
//! Provides `RegexPiiDetector` and `RegexPiiRedactor` implementations
//! using compiled regex patterns for common PII categories.

use async_trait::async_trait;
use mofa_kernel::security::{
    PiiDetector, PiiRedactor, RedactionMatch, RedactionResult, RedactionStrategy, SecurityResult,
    SensitiveDataCategory,
};
use once_cell::sync::Lazy;
use regex::Regex;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use tracing::warn;


// =============================================================================
// Compiled Regex Patterns
// =============================================================================

// Email: RFC 5322 simplified
static EMAIL_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"[a-zA-Z0-9._%+-]+@[a-zA-Z0-9.-]+\.[a-zA-Z]{2,}").unwrap());

// Phone: US formats (xxx-xxx-xxxx, (xxx) xxx-xxxx, +1xxxxxxxxxx, etc.)
static PHONE_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?:\+?1[-.\s]?)?\(?[2-9]\d{2}\)?[-.\s]?\d{3}[-.\s]?\d{4}").unwrap()
});

// Credit card: 13-19 digit sequences (with optional separators)
static CREDIT_CARD_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"\b\d{4}[-\s]?\d{4}[-\s]?\d{4}[-\s]?\d{1,7}\b").unwrap());

// SSN: xxx-xx-xxxx
static SSN_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"\b\d{3}-\d{2}-\d{4}\b").unwrap());

// IPv4
static IPV4_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"\b(?:(?:25[0-5]|2[0-4]\d|[01]?\d\d?)\.){3}(?:25[0-5]|2[0-4]\d|[01]?\d\d?)\b")
        .unwrap()
});

// IPv6 (simplified: 8 groups of hex or with :: abbreviation)
static IPV6_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?i)(?:[0-9a-f]{1,4}:){7}[0-9a-f]{1,4}|(?:[0-9a-f]{1,4}:){1,7}:|(?:[0-9a-f]{1,4}:){1,6}:[0-9a-f]{1,4}|::(?:[0-9a-f]{1,4}:){0,5}[0-9a-f]{1,4}|::")
        .unwrap()
});

// API keys: common formats (sk-..., ghp_..., AKIA..., xoxb-...)
static API_KEY_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"\b(?:sk-[a-zA-Z0-9]{20,}|ghp_[a-zA-Z0-9]{36,}|AKIA[A-Z0-9]{16}|xoxb-[a-zA-Z0-9-]+)\b")
        .unwrap()
});

// =============================================================================
// Luhn Validation
// =============================================================================

/// Validate a credit card number using the Luhn algorithm.
fn luhn_check(number: &str) -> bool {
    let digits: Vec<u32> = number
        .chars()
        .filter(|c| c.is_ascii_digit())
        .filter_map(|c| c.to_digit(10))
        .collect();

    if digits.len() < 13 {
        return false;
    }

    let mut sum = 0u32;
    let mut double = false;

    for &digit in digits.iter().rev() {
        let mut d = digit;
        if double {
            d *= 2;
            if d > 9 {
                d -= 9;
            }
        }
        sum += d;
        double = !double;
    }

    sum % 10 == 0
}

// =============================================================================
// RegexPiiDetector
// =============================================================================

/// Regex-based PII detector.
///
/// Detects common PII patterns using compiled regex. Only scans for
/// categories explicitly listed in `enabled_categories`.
#[derive(Debug, Clone)]
pub struct RegexPiiDetector {
    /// Categories to detect
    pub enabled_categories: Vec<SensitiveDataCategory>,
}

impl Default for RegexPiiDetector {
    fn default() -> Self {
        Self {
            enabled_categories: vec![
                SensitiveDataCategory::Email,
                SensitiveDataCategory::Phone,
                SensitiveDataCategory::CreditCard,
                SensitiveDataCategory::Ssn,
                SensitiveDataCategory::IpAddress,
                SensitiveDataCategory::ApiKey,
            ],
        }
    }
}

impl RegexPiiDetector {
    /// Create a detector for specific categories only.
    #[must_use]
    pub fn with_categories(categories: Vec<SensitiveDataCategory>) -> Self {
        Self {
            enabled_categories: categories,
        }
    }

    fn get_regex(category: &SensitiveDataCategory) -> Option<Vec<&'static Regex>> {
        match category {
            SensitiveDataCategory::Email => Some(vec![&EMAIL_RE]),
            SensitiveDataCategory::Phone => Some(vec![&PHONE_RE]),
            SensitiveDataCategory::CreditCard => Some(vec![&CREDIT_CARD_RE]),
            SensitiveDataCategory::Ssn => Some(vec![&SSN_RE]),
            SensitiveDataCategory::IpAddress => Some(vec![&IPV4_RE, &IPV6_RE]),
            SensitiveDataCategory::ApiKey => Some(vec![&API_KEY_RE]),
            SensitiveDataCategory::Custom(name) => {
                warn!(category = %name, "Custom PII category is not supported by RegexPiiDetector");
                None
            }
            _ => None,
        }
    }
}

#[async_trait]
impl PiiDetector for RegexPiiDetector {
    async fn detect(&self, text: &str) -> SecurityResult<Vec<RedactionMatch>> {
        let mut matches = Vec::new();

        for category in &self.enabled_categories {
            if let Some(regexes) = Self::get_regex(category) {
                for re in regexes {
                    for m in re.find_iter(text) {
                        // Extra validation for credit cards (Luhn check)
                        if *category == SensitiveDataCategory::CreditCard && !luhn_check(m.as_str()) {
                            continue;
                        }

                        matches.push(RedactionMatch {
                            category: category.clone(),
                            start: m.start(),
                            end: m.end(),
                            original: m.as_str().to_string(),
                            replacement: String::new(), // Will be filled by redactor
                        });
                    }
                }
            }
        }

        // Sort by start position for deterministic output
        matches.sort_by_key(|m| m.start);
        Ok(matches)
    }
}

// =============================================================================
// RegexPiiRedactor
// =============================================================================

/// Regex-based PII redactor.
///
/// Detects and redacts PII in a single operation. Uses `RegexPiiDetector`
/// for detection and applies the specified `RedactionStrategy`.
#[derive(Debug, Clone, Default)]
pub struct RegexPiiRedactor {
    detector: RegexPiiDetector,
}

impl RegexPiiRedactor {
    /// Create a redactor for specific categories only.
    #[must_use]
    pub fn with_categories(categories: Vec<SensitiveDataCategory>) -> Self {
        Self {
            detector: RegexPiiDetector::with_categories(categories),
        }
    }

    fn apply_strategy(original: &str, category: &SensitiveDataCategory, strategy: &RedactionStrategy) -> String {
        match strategy {
            RedactionStrategy::Mask => Self::mask_value(original, category),
            RedactionStrategy::Hash => {
                let mut hasher = DefaultHasher::new();
                original.hash(&mut hasher);
                let hash = hasher.finish();
                format!("[{:08x}]", hash as u32)
            }
            RedactionStrategy::Remove => String::new(),
            RedactionStrategy::Replace(placeholder) => placeholder.clone(),
            _ => "[REDACTED]".to_string(),
        }
    }

    fn mask_value(original: &str, category: &SensitiveDataCategory) -> String {
        match category {
            SensitiveDataCategory::Email => {
                // Preserve first char and domain: j***@example.com
                if let Some(at_pos) = original.find('@') {
                    let first_char = original.chars().next().unwrap_or('*');
                    let domain = &original[at_pos..];
                    format!("{first_char}***{domain}")
                } else {
                    "***@***".to_string()
                }
            }
            SensitiveDataCategory::Phone => {
                // Show last 4 digits: ***-***-1234
                let digits: String = original.chars().filter(|c| c.is_ascii_digit()).collect();
                if digits.len() >= 4 {
                    format!("***-***-{}", &digits[digits.len() - 4..])
                } else {
                    "***-***-****".to_string()
                }
            }
            SensitiveDataCategory::CreditCard => {
                // Show last 4 digits: ****-****-****-1234
                let digits: String = original.chars().filter(|c| c.is_ascii_digit()).collect();
                if digits.len() >= 4 {
                    format!("****-****-****-{}", &digits[digits.len() - 4..])
                } else {
                    "****-****-****-****".to_string()
                }
            }
            SensitiveDataCategory::Ssn => "***-**-****".to_string(),
            SensitiveDataCategory::IpAddress => "***.***.***.***".to_string(),
            SensitiveDataCategory::ApiKey => {
                // Show prefix: sk-****
                let prefix: String = original.chars().take(3).collect();
                format!("{prefix}****")
            }
            SensitiveDataCategory::Custom(_) | _ => "*".repeat(original.len()),
        }
    }
}

#[async_trait]
impl PiiRedactor for RegexPiiRedactor {
    async fn redact(
        &self,
        text: &str,
        strategy: &RedactionStrategy,
    ) -> SecurityResult<RedactionResult> {
        let mut detections = self.detector.detect(text).await?;

        // Fill in replacements
        for detection in &mut detections {
            detection.replacement =
                Self::apply_strategy(&detection.original, &detection.category, strategy);
        }

        // Build redacted text by replacing matches in reverse order
        let mut redacted = text.to_string();
        for detection in detections.iter().rev() {
            redacted.replace_range(detection.start..detection.end, &detection.replacement);
        }

        Ok(RedactionResult {
            original_text: text.to_string(),
            redacted_text: redacted,
            matches: detections,
        })
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // --- Luhn tests ---

    #[test]
    fn luhn_valid_cards() {
        assert!(luhn_check("4111111111111111")); // Visa test
        assert!(luhn_check("5500000000000004")); // Mastercard test
        assert!(luhn_check("378282246310005")); // Amex test
    }

    #[test]
    fn luhn_invalid_cards() {
        assert!(!luhn_check("4111111111111112")); // Off by one
        assert!(!luhn_check("1234567890123456")); // Random
        assert!(!luhn_check("12345")); // Too short
    }

    // --- Detection tests ---

    #[tokio::test]
    async fn detect_email() {
        let detector = RegexPiiDetector::with_categories(vec![SensitiveDataCategory::Email]);
        let matches = detector.detect("Contact: john@example.com or support@test.org").await.unwrap();
        assert_eq!(matches.len(), 2);
        assert_eq!(matches[0].original, "john@example.com");
        assert_eq!(matches[1].original, "support@test.org");
    }

    #[tokio::test]
    async fn detect_phone() {
        let detector = RegexPiiDetector::with_categories(vec![SensitiveDataCategory::Phone]);
        let matches = detector.detect("Call 555-123-4567 or (555) 987-6543").await.unwrap();
        assert_eq!(matches.len(), 2);
    }

    #[tokio::test]
    async fn detect_credit_card_with_luhn() {
        let detector =
            RegexPiiDetector::with_categories(vec![SensitiveDataCategory::CreditCard]);
        // Valid Visa test number
        let matches = detector.detect("Card: 4111 1111 1111 1111").await.unwrap();
        assert_eq!(matches.len(), 1);

        // Invalid number should not match
        let matches = detector.detect("Card: 4111 1111 1111 1112").await.unwrap();
        assert_eq!(matches.len(), 0);
    }

    #[tokio::test]
    async fn detect_ssn() {
        let detector = RegexPiiDetector::with_categories(vec![SensitiveDataCategory::Ssn]);
        let matches = detector.detect("SSN: 123-45-6789").await.unwrap();
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].original, "123-45-6789");
    }

    #[tokio::test]
    async fn detect_ip_address() {
        let detector = RegexPiiDetector::with_categories(vec![SensitiveDataCategory::IpAddress]);
        let matches = detector.detect("Server at 192.168.1.1 and 10.0.0.1").await.unwrap();
        assert_eq!(matches.len(), 2);
    }

    #[tokio::test]
    async fn detect_api_key() {
        let detector = RegexPiiDetector::with_categories(vec![SensitiveDataCategory::ApiKey]);
        let matches = detector
            .detect("Key: sk-abcdefghijklmnopqrstuvwxyz1234567890")
            .await
            .unwrap();
        assert_eq!(matches.len(), 1);
    }

    #[tokio::test]
    async fn no_false_positives_on_normal_text() {
        let detector = RegexPiiDetector::default();
        let matches = detector
            .detect("Hello, this is a normal sentence with no PII at all.")
            .await
            .unwrap();
        assert_eq!(matches.len(), 0);
    }

    #[tokio::test]
    async fn detect_multiple_categories() {
        let detector = RegexPiiDetector::default();
        let matches = detector
            .detect("Email: test@example.com, SSN: 123-45-6789, IP: 10.0.0.1")
            .await
            .unwrap();
        assert!(matches.len() >= 3);
    }

    // --- Redaction tests ---

    #[tokio::test]
    async fn redact_email_mask() {
        let redactor = RegexPiiRedactor::with_categories(vec![SensitiveDataCategory::Email]);
        let result = redactor
            .redact("Email: john@example.com", &RedactionStrategy::Mask)
            .await
            .unwrap();
        assert_eq!(result.redacted_text, "Email: j***@example.com");
        assert!(result.has_redactions());
    }

    #[tokio::test]
    async fn redact_ssn_mask() {
        let redactor = RegexPiiRedactor::with_categories(vec![SensitiveDataCategory::Ssn]);
        let result = redactor
            .redact("SSN: 123-45-6789", &RedactionStrategy::Mask)
            .await
            .unwrap();
        assert_eq!(result.redacted_text, "SSN: ***-**-****");
    }

    #[tokio::test]
    async fn redact_remove_strategy() {
        let redactor = RegexPiiRedactor::with_categories(vec![SensitiveDataCategory::Email]);
        let result = redactor
            .redact("Email: test@example.com done", &RedactionStrategy::Remove)
            .await
            .unwrap();
        assert_eq!(result.redacted_text, "Email:  done");
    }

    #[tokio::test]
    async fn redact_replace_strategy() {
        let redactor = RegexPiiRedactor::with_categories(vec![SensitiveDataCategory::Email]);
        let result = redactor
            .redact(
                "Email: test@example.com",
                &RedactionStrategy::Replace("[REDACTED]".into()),
            )
            .await
            .unwrap();
        assert_eq!(result.redacted_text, "Email: [REDACTED]");
    }

    #[tokio::test]
    async fn redact_hash_strategy() {
        let redactor = RegexPiiRedactor::with_categories(vec![SensitiveDataCategory::Ssn]);
        let result = redactor
            .redact("SSN: 123-45-6789", &RedactionStrategy::Hash)
            .await
            .unwrap();
        assert!(result.redacted_text.starts_with("SSN: ["));
        assert!(result.redacted_text.ends_with(']'));
        assert_ne!(result.redacted_text, "SSN: 123-45-6789");
    }

    #[tokio::test]
    async fn redact_no_match_returns_unchanged() {
        let redactor = RegexPiiRedactor::default();
        let result = redactor
            .redact("No PII here", &RedactionStrategy::Mask)
            .await
            .unwrap();
        assert_eq!(result.redacted_text, "No PII here");
        assert!(!result.has_redactions());
    }

    #[tokio::test]
    async fn redact_credit_card_mask() {
        let redactor =
            RegexPiiRedactor::with_categories(vec![SensitiveDataCategory::CreditCard]);
        let result = redactor
            .redact("Card: 4111111111111111", &RedactionStrategy::Mask)
            .await
            .unwrap();
        assert_eq!(result.redacted_text, "Card: ****-****-****-1111");
    }
}
