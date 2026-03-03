//! PII Redactor Implementation
//!
//! Redacts sensitive data from text using configurable strategies.

use crate::security::pii::detector::RegexPiiDetector;
use async_trait::async_trait;
use mofa_runtime::security::error::{SecurityError, SecurityResult};
use mofa_runtime::security::traits::{PiiDetector, PiiRedactor, RedactionResult};
use mofa_runtime::security::types::{RedactionStrategy, SensitiveDataCategory};
use sha2::{Digest, Sha256};
use std::collections::HashSet;

/// Regex-based PII redactor
pub struct RegexPiiRedactor {
    detector: RegexPiiDetector,
    /// Default strategy for redaction
    default_strategy: RedactionStrategy,
    /// Per-category strategies (overrides default)
    category_strategies: std::collections::HashMap<SensitiveDataCategory, RedactionStrategy>,
}

impl RegexPiiRedactor {
    /// Create a new RegexPiiRedactor with default mask strategy
    pub fn new() -> Self {
        Self {
            detector: RegexPiiDetector::new(),
            default_strategy: RedactionStrategy::Mask,
            category_strategies: std::collections::HashMap::new(),
        }
    }

    /// Set the default redaction strategy
    pub fn with_default_strategy(mut self, strategy: RedactionStrategy) -> Self {
        self.default_strategy = strategy;
        self
    }

    /// Set a specific strategy for a category
    pub fn with_category_strategy(
        mut self,
        category: SensitiveDataCategory,
        strategy: RedactionStrategy,
    ) -> Self {
        self.category_strategies.insert(category, strategy);
        self
    }

    /// Get the strategy for a specific category
    fn get_strategy(&self, category: &SensitiveDataCategory) -> RedactionStrategy {
        self.category_strategies
            .get(category)
            .copied()
            .unwrap_or(self.default_strategy)
    }

    /// Redact a single PII value
    fn redact_value(&self, value: &str, category: &SensitiveDataCategory) -> String {
        let strategy = self.get_strategy(category);
        match strategy {
            RedactionStrategy::Mask => "[REDACTED]".to_string(),
            RedactionStrategy::Hash => {
                let mut hasher = Sha256::new();
                hasher.update(value.as_bytes());
                let hash = hasher.finalize();
                format!("[HASH:{}]", hex::encode(&hash[..8]))
            }
            RedactionStrategy::Remove => "".to_string(),
            RedactionStrategy::Replace => {
                // For Replace strategy, use a category-specific placeholder
                match category {
                    SensitiveDataCategory::Email => "[EMAIL]".to_string(),
                    SensitiveDataCategory::Phone => "[PHONE]".to_string(),
                    SensitiveDataCategory::CreditCard => "[CARD]".to_string(),
                    SensitiveDataCategory::SSN => "[SSN]".to_string(),
                    SensitiveDataCategory::IpAddress => "[IP]".to_string(),
                    SensitiveDataCategory::ApiKey => "[API_KEY]".to_string(),
                    SensitiveDataCategory::Custom(name) => format!("[{}]", name.to_uppercase()),
                    _ => "[REDACTED]".to_string(), // Fallback for any future categories
                }
            }
            _ => "[REDACTED]".to_string(), // Fallback for future strategies
        }
    }
}

impl Default for RegexPiiRedactor {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl PiiRedactor for RegexPiiRedactor {
    async fn redact(&self, text: &str, strategy: RedactionStrategy) -> SecurityResult<RedactionResult> {
        // Detect all PII
        let detections = self.detector.detect(text).await?;

        if detections.is_empty() {
            return Ok(RedactionResult::new(
                text.to_string(),
                0,
                Vec::new(),
            ));
        }

        // Use provided strategy as override for this call
        let mut redacted_text = text.to_string();
        let mut redaction_count = 0;
        let mut redacted_categories = HashSet::new();

        // Process detections in reverse order to maintain correct indices
        for detection in detections.iter().rev() {
            redacted_categories.insert(detection.category.name().to_string());
            
            // Use the provided strategy for this call
            let effective_strategy = strategy;
            
            let replacement = match effective_strategy {
                RedactionStrategy::Mask => "[REDACTED]".to_string(),
                RedactionStrategy::Hash => {
                    let mut hasher = Sha256::new();
                    hasher.update(detection.value.as_bytes());
                    let hash = hasher.finalize();
                    format!("[HASH:{}]", hex::encode(&hash[..8]))
                }
                RedactionStrategy::Remove => "".to_string(),
                RedactionStrategy::Replace => self.redact_value(&detection.value, &detection.category),
                _ => "[REDACTED]".to_string(), // Fallback for future strategies
            };

            // Replace in reverse order to maintain indices
            redacted_text.replace_range(detection.start..detection.end, &replacement);
            redaction_count += 1;
        }

        Ok(RedactionResult::new(
            redacted_text,
            redaction_count,
            redacted_categories.into_iter().collect(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_redact_mask() {
        let redactor = RegexPiiRedactor::new();
        let text = "Email: user@example.com";
        let result = redactor.redact(text, RedactionStrategy::Mask).await.unwrap();

        assert_eq!(result.redaction_count, 1);
        assert!(result.text.contains("[REDACTED]"));
        assert!(!result.text.contains("user@example.com"));
    }

    #[tokio::test]
    async fn test_redact_hash() {
        let redactor = RegexPiiRedactor::new();
        let text = "Email: user@example.com";
        let result = redactor.redact(text, RedactionStrategy::Hash).await.unwrap();

        assert_eq!(result.redaction_count, 1);
        assert!(result.text.contains("[HASH:"));
        assert!(!result.text.contains("user@example.com"));
    }

    #[tokio::test]
    async fn test_redact_remove() {
        let redactor = RegexPiiRedactor::new();
        let text = "Email: user@example.com";
        let result = redactor.redact(text, RedactionStrategy::Remove).await.unwrap();

        assert_eq!(result.redaction_count, 1);
        assert!(!result.text.contains("user@example.com"));
    }

    #[tokio::test]
    async fn test_redact_multiple() {
        let redactor = RegexPiiRedactor::new();
        let text = "Email: user@example.com, Phone: (555) 123-4567";
        let result = redactor.redact(text, RedactionStrategy::Mask).await.unwrap();

        assert_eq!(result.redaction_count, 2);
        assert_eq!(result.redacted_categories.len(), 2);
    }

    #[tokio::test]
    async fn test_redact_no_pii() {
        let redactor = RegexPiiRedactor::new();
        let text = "No sensitive data here";
        let result = redactor.redact(text, RedactionStrategy::Mask).await.unwrap();

        assert_eq!(result.redaction_count, 0);
        assert_eq!(result.text, text);
    }
}
