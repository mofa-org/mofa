//! PII Detector Implementation
//!
//! Regex-based PII detection using compiled patterns.

use crate::security::pii::patterns::{
    API_KEY_PATTERN, CREDIT_CARD_PATTERN, EMAIL_PATTERN, IP_ADDRESS_PATTERN, PHONE_PATTERN,
    SSN_PATTERN, validate_luhn,
};
use async_trait::async_trait;
use mofa_runtime::security::error::{SecurityError, SecurityResult};
use mofa_runtime::security::traits::PiiDetector;
use mofa_runtime::security::types::{DetectedPii, SensitiveDataCategory};

/// Regex-based PII detector
pub struct RegexPiiDetector {
    /// Whether to validate credit cards with Luhn algorithm
    validate_credit_cards: bool,
}

impl RegexPiiDetector {
    /// Create a new RegexPiiDetector
    pub fn new() -> Self {
        Self {
            validate_credit_cards: true,
        }
    }

    /// Create a detector that doesn't validate credit cards (faster but less accurate)
    pub fn without_validation() -> Self {
        Self {
            validate_credit_cards: false,
        }
    }

    /// Set whether to validate credit cards
    pub fn with_credit_card_validation(mut self, validate: bool) -> Self {
        self.validate_credit_cards = validate;
        self
    }

    /// Detect email addresses
    fn detect_emails(&self, text: &str) -> Vec<DetectedPii> {
        EMAIL_PATTERN
            .find_iter(text)
            .map(|m| DetectedPii {
                category: SensitiveDataCategory::Email,
                value: m.as_str().to_string(),
                start: m.start(),
                end: m.end(),
            })
            .collect()
    }

    /// Detect phone numbers
    fn detect_phones(&self, text: &str) -> Vec<DetectedPii> {
        PHONE_PATTERN
            .find_iter(text)
            .map(|m| DetectedPii {
                category: SensitiveDataCategory::Phone,
                value: m.as_str().to_string(),
                start: m.start(),
                end: m.end(),
            })
            .collect()
    }

    /// Detect credit card numbers
    fn detect_credit_cards(&self, text: &str) -> Vec<DetectedPii> {
        CREDIT_CARD_PATTERN
            .find_iter(text)
            .filter(|m| {
                if self.validate_credit_cards {
                    // Remove spaces and dashes for validation
                    let cleaned: String = m.as_str().chars().filter(|c| c.is_ascii_digit()).collect();
                    validate_luhn(&cleaned)
                } else {
                    true
                }
            })
            .map(|m| DetectedPii {
                category: SensitiveDataCategory::CreditCard,
                value: m.as_str().to_string(),
                start: m.start(),
                end: m.end(),
            })
            .collect()
    }

    /// Detect SSNs
    fn detect_ssns(&self, text: &str) -> Vec<DetectedPii> {
        SSN_PATTERN
            .find_iter(text)
            .map(|m| DetectedPii {
                category: SensitiveDataCategory::SSN,
                value: m.as_str().to_string(),
                start: m.start(),
                end: m.end(),
            })
            .collect()
    }

    /// Detect IP addresses
    fn detect_ip_addresses(&self, text: &str) -> Vec<DetectedPii> {
        IP_ADDRESS_PATTERN
            .find_iter(text)
            .filter(|m| {
                // Basic validation: check if each octet is <= 255
                let parts: Vec<&str> = m.as_str().split('.').collect();
                if parts.len() != 4 {
                    return false;
                }
                parts.iter().all(|part| {
                    part.parse::<u8>().is_ok()
                })
            })
            .map(|m| DetectedPii {
                category: SensitiveDataCategory::IpAddress,
                value: m.as_str().to_string(),
                start: m.start(),
                end: m.end(),
            })
            .collect()
    }

    /// Detect API keys
    fn detect_api_keys(&self, text: &str) -> Vec<DetectedPii> {
        API_KEY_PATTERN
            .find_iter(text)
            .map(|m| DetectedPii {
                category: SensitiveDataCategory::ApiKey,
                value: m.as_str().to_string(),
                start: m.start(),
                end: m.end(),
            })
            .collect()
    }
}

impl Default for RegexPiiDetector {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl PiiDetector for RegexPiiDetector {
    async fn detect(&self, text: &str) -> SecurityResult<Vec<DetectedPii>> {
        let mut all_detections = Vec::new();

        // Run all detectors
        all_detections.extend(self.detect_emails(text));
        all_detections.extend(self.detect_phones(text));
        all_detections.extend(self.detect_credit_cards(text));
        all_detections.extend(self.detect_ssns(text));
        all_detections.extend(self.detect_ip_addresses(text));
        all_detections.extend(self.detect_api_keys(text));

        // Sort by start position
        all_detections.sort_by_key(|d| d.start);

        Ok(all_detections)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_detect_emails() {
        let detector = RegexPiiDetector::new();
        let text = "Contact me at user@example.com or admin@test.org";
        let results = detector.detect(text).await.unwrap();

        assert_eq!(results.len(), 2);
        assert_eq!(results[0].category, SensitiveDataCategory::Email);
        assert_eq!(results[0].value, "user@example.com");
    }

    #[tokio::test]
    async fn test_detect_phones() {
        let detector = RegexPiiDetector::new();
        let text = "Call me at (555) 123-4567 or 555-987-6543";
        let results = detector.detect(text).await.unwrap();

        assert!(results.len() >= 2);
        assert_eq!(results[0].category, SensitiveDataCategory::Phone);
    }

    #[tokio::test]
    async fn test_detect_credit_cards() {
        let detector = RegexPiiDetector::new();
        let text = "Card: 4111-1111-1111-1111";
        let results = detector.detect(text).await.unwrap();

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].category, SensitiveDataCategory::CreditCard);
    }

    #[tokio::test]
    async fn test_detect_ssns() {
        let detector = RegexPiiDetector::new();
        let text = "SSN: 123-45-6789";
        let results = detector.detect(text).await.unwrap();

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].category, SensitiveDataCategory::SSN);
    }

    #[tokio::test]
    async fn test_detect_multiple_types() {
        let detector = RegexPiiDetector::new();
        let text = "Email: user@example.com, Phone: (555) 123-4567, SSN: 123-45-6789";
        let results = detector.detect(text).await.unwrap();

        assert_eq!(results.len(), 3);
        assert_eq!(results[0].category, SensitiveDataCategory::Email);
        assert_eq!(results[1].category, SensitiveDataCategory::Phone);
        assert_eq!(results[2].category, SensitiveDataCategory::SSN);
    }

    #[tokio::test]
    async fn test_no_pii() {
        let detector = RegexPiiDetector::new();
        let text = "This is just regular text with no sensitive information.";
        let results = detector.detect(text).await.unwrap();

        assert_eq!(results.len(), 0);
    }
}
