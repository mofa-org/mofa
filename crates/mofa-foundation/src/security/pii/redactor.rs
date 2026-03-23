//! PII Redactor Implementation
//!
//! Redacts sensitive data from text using configurable strategies.

use crate::security::pii::detector::RegexPiiDetector;
use async_trait::async_trait;
use mofa_kernel::security::{
    PiiDetector, PiiRedactor, RedactionMatch, RedactionResult, RedactionStrategy, SecurityResult,
    SensitiveDataCategory,
};
use sha2::{Digest, Sha256};

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
    fn get_strategy(&self, category: &SensitiveDataCategory) -> &RedactionStrategy {
        self.category_strategies
            .get(category)
            .unwrap_or(&self.default_strategy)
    }

    /// Redact a single PII value
    fn redact_value(
        &self,
        value: &str,
        category: &SensitiveDataCategory,
        strategy: &RedactionStrategy,
    ) -> String {
        match strategy {
            RedactionStrategy::Mask => "[REDACTED]".to_string(),
            RedactionStrategy::Hash => {
                let mut hasher = Sha256::new();
                hasher.update(value.as_bytes());
                let hash = hasher.finalize();
                format!("[HASH:{}]", hex::encode(&hash[..8]))
            }
            RedactionStrategy::Remove => "".to_string(),
            RedactionStrategy::Replace(placeholder) => placeholder.clone(),
            _ => "[REDACTED]".to_string(), // Handle future variants
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
    async fn redact(
        &self,
        text: &str,
        strategy: &RedactionStrategy,
    ) -> SecurityResult<RedactionResult> {
        // Detect all PII
        let mut matches = self.detector.detect(text).await?;

        if matches.is_empty() {
            return Ok(RedactionResult {
                original_text: text.to_string(),
                redacted_text: text.to_string(),
                matches: Vec::new(),
            });
        }

        // Use provided strategy as override for this call
        let mut redacted_text = text.to_string();

        // Process matches in reverse order to maintain correct indices
        for match_item in matches.iter_mut().rev() {
            // Get the strategy for this category (or use provided strategy)
            let effective_strategy = self
                .category_strategies
                .get(&match_item.category)
                .unwrap_or(strategy);

            let replacement = self.redact_value(
                &match_item.original,
                &match_item.category,
                effective_strategy,
            );

            // Update the match with the replacement
            match_item.replacement = replacement.clone();

            // Replace in reverse order to maintain indices
            redacted_text.replace_range(match_item.start..match_item.end, &replacement);
        }

        // Reverse matches back to original order
        matches.reverse();

        Ok(RedactionResult {
            original_text: text.to_string(),
            redacted_text,
            matches,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    #[tokio::test]
    async fn test_redact_mask() {
        let redactor = RegexPiiRedactor::new();
        let text = "Email: user@example.com";
        let result = redactor
            .redact(text, &RedactionStrategy::Mask)
            .await
            .unwrap();

        assert_eq!(result.matches.len(), 1);
        assert!(result.redacted_text.contains("[REDACTED]"));
        assert!(!result.redacted_text.contains("user@example.com"));
    }

    #[tokio::test]
    async fn test_redact_hash() {
        let redactor = RegexPiiRedactor::new();
        let text = "Email: user@example.com";
        let result = redactor
            .redact(text, &RedactionStrategy::Hash)
            .await
            .unwrap();

        assert_eq!(result.matches.len(), 1);
        assert!(result.redacted_text.contains("[HASH:"));
        assert!(!result.redacted_text.contains("user@example.com"));
    }

    #[tokio::test]
    async fn test_redact_remove() {
        let redactor = RegexPiiRedactor::new();
        let text = "Email: user@example.com";
        let result = redactor
            .redact(text, &RedactionStrategy::Remove)
            .await
            .unwrap();

        assert_eq!(result.matches.len(), 1);
        assert!(!result.redacted_text.contains("user@example.com"));
    }

    #[tokio::test]
    async fn test_redact_no_pii() {
        let redactor = RegexPiiRedactor::new();
        let text = "No sensitive data here";
        let result = redactor
            .redact(text, &RedactionStrategy::Mask)
            .await
            .unwrap();

        assert_eq!(result.matches.len(), 0);
        assert_eq!(result.redacted_text, text);
    }

    #[tokio::test]
    async fn test_category_strategy_overrides_default() {
        let redactor = RegexPiiRedactor::new().with_category_strategy(
            SensitiveDataCategory::Email,
            RedactionStrategy::Replace("[EMAIL]".to_string()),
        );
        let text = "Email: user@example.com";
        let result = redactor
            .redact(text, &RedactionStrategy::Mask)
            .await
            .unwrap();

        assert_eq!(result.matches.len(), 1);
        assert!(result.redacted_text.contains("[EMAIL]"));
        assert!(!result.redacted_text.contains("[REDACTED]"));
    }

    #[tokio::test]
    async fn test_redact_empty_input() {
        let redactor = RegexPiiRedactor::new();
        let result = redactor.redact("", &RedactionStrategy::Mask).await.unwrap();

        assert_eq!(result.matches.len(), 0);
        assert_eq!(result.original_text, "");
        assert_eq!(result.redacted_text, "");
    }

    #[tokio::test]
    async fn test_redact_repeated_pii() {
        let redactor = RegexPiiRedactor::new();
        let text = "user@example.com and user@example.com";
        let result = redactor
            .redact(text, &RedactionStrategy::Mask)
            .await
            .unwrap();

        assert_eq!(result.matches.len(), 2);
        assert_eq!(result.redacted_text, "[REDACTED] and [REDACTED]");
    }

    #[tokio::test]
    async fn test_concurrent_redaction_consistency() {
        let redactor = Arc::new(RegexPiiRedactor::new());
        let text = "Email: user@example.com, Phone: (555) 123-4567";

        let mut tasks = Vec::new();
        for _ in 0..16 {
            let redactor = Arc::clone(&redactor);
            let text = text.to_string();
            tasks.push(tokio::spawn(async move {
                redactor
                    .redact(&text, &RedactionStrategy::Mask)
                    .await
                    .unwrap()
            }));
        }

        for task in tasks {
            let result = task.await.unwrap();
            assert_eq!(result.matches.len(), 2);
            assert!(!result.redacted_text.contains("user@example.com"));
            assert!(!result.redacted_text.contains("123-4567"));
            assert_eq!(result.redacted_text.matches("[REDACTED]").count(), 2);
        }
    }

    #[tokio::test]
    async fn test_redact_replace_strategy() {
        let redactor = RegexPiiRedactor::new();
        let text = "Email: user@example.com";
        let result = redactor
            .redact(text, &RedactionStrategy::Replace("[EMAIL]".to_string()))
            .await
            .unwrap();

        assert_eq!(result.matches.len(), 1);
        assert_eq!(result.redacted_text, "Email: [EMAIL]");
    }

    #[tokio::test]
    async fn test_redact_hash_is_deterministic() {
        let redactor = RegexPiiRedactor::new();
        let text = "Email: user@example.com";

        let first = redactor
            .redact(text, &RedactionStrategy::Hash)
            .await
            .unwrap();
        let second = redactor
            .redact(text, &RedactionStrategy::Hash)
            .await
            .unwrap();

        assert_eq!(first.redacted_text, second.redacted_text);
    }

    #[tokio::test]
    async fn test_category_strategy_applies_only_to_configured_category() {
        let redactor = RegexPiiRedactor::new().with_category_strategy(
            SensitiveDataCategory::Email,
            RedactionStrategy::Replace("[EMAIL]".to_string()),
        );
        let text = "Email: user@example.com, SSN: 123-45-6789";
        let result = redactor
            .redact(text, &RedactionStrategy::Mask)
            .await
            .unwrap();

        assert!(result.redacted_text.contains("[EMAIL]"));
        assert!(result.redacted_text.contains("[REDACTED]"));
    }

    #[tokio::test]
    async fn test_match_replacement_field_is_populated() {
        let redactor = RegexPiiRedactor::new();
        let text = "Email: user@example.com";
        let result = redactor
            .redact(text, &RedactionStrategy::Mask)
            .await
            .unwrap();

        assert_eq!(result.matches.len(), 1);
        assert_eq!(result.matches[0].replacement, "[REDACTED]");
    }

    #[tokio::test]
    async fn test_original_text_is_preserved_in_result() {
        let redactor = RegexPiiRedactor::new();
        let text = "Email: user@example.com";
        let result = redactor
            .redact(text, &RedactionStrategy::Mask)
            .await
            .unwrap();

        assert_eq!(result.original_text, text);
    }
}
