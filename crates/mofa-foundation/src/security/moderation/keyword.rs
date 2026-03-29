//! Keyword-based Content Moderator
//!
//! Simple keyword-based content moderation using configurable word lists.

use async_trait::async_trait;
use mofa_kernel::security::{
    ContentModerator, ContentPolicy, ModerationCategory, ModerationVerdict, SecurityResult,
};
use std::collections::HashSet;

/// Keyword-based content moderator
pub struct KeywordModerator {
    /// Blocked keywords (case-insensitive)
    blocked_keywords: HashSet<String>,
    /// Flagged keywords (case-insensitive)
    flagged_keywords: HashSet<String>,
    /// Whether to use case-sensitive matching
    case_sensitive: bool,
}

impl KeywordModerator {
    /// Create a new KeywordModerator
    pub fn new() -> Self {
        Self {
            blocked_keywords: HashSet::new(),
            flagged_keywords: HashSet::new(),
            case_sensitive: false,
        }
    }

    /// Add a blocked keyword
    pub fn add_blocked(mut self, keyword: impl Into<String>) -> Self {
        let keyword = keyword.into();
        if self.case_sensitive {
            self.blocked_keywords.insert(keyword);
        } else {
            self.blocked_keywords.insert(keyword.to_lowercase());
        }
        self
    }

    /// Add multiple blocked keywords
    pub fn add_blocked_many(mut self, keywords: impl IntoIterator<Item = String>) -> Self {
        for keyword in keywords {
            self = self.add_blocked(keyword);
        }
        self
    }

    /// Add a flagged keyword
    pub fn add_flagged(mut self, keyword: impl Into<String>) -> Self {
        let keyword = keyword.into();
        if self.case_sensitive {
            self.flagged_keywords.insert(keyword);
        } else {
            self.flagged_keywords.insert(keyword.to_lowercase());
        }
        self
    }

    /// Add multiple flagged keywords
    pub fn add_flagged_many(mut self, keywords: impl IntoIterator<Item = String>) -> Self {
        for keyword in keywords {
            self = self.add_flagged(keyword);
        }
        self
    }

    /// Set case sensitivity
    pub fn with_case_sensitive(mut self, case_sensitive: bool) -> Self {
        self.case_sensitive = case_sensitive;
        self
    }

    /// Check if content contains blocked keywords
    fn check_blocked(&self, content: &str) -> Option<String> {
        let check_content = if self.case_sensitive {
            content.to_string()
        } else {
            content.to_lowercase()
        };

        for keyword in &self.blocked_keywords {
            if check_content.contains(keyword) {
                return Some(format!("Contains blocked keyword: {}", keyword));
            }
        }
        None
    }

    /// Check if content contains flagged keywords
    fn check_flagged(&self, content: &str) -> Option<String> {
        let check_content = if self.case_sensitive {
            content.to_string()
        } else {
            content.to_lowercase()
        };

        for keyword in &self.flagged_keywords {
            if check_content.contains(keyword) {
                return Some(format!("Contains flagged keyword: {}", keyword));
            }
        }
        None
    }
}

impl Default for KeywordModerator {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl ContentModerator for KeywordModerator {
    async fn moderate(
        &self,
        content: &str,
        policy: &ContentPolicy,
    ) -> SecurityResult<ModerationVerdict> {
        // Check if moderation is enabled for this policy
        if !policy
            .enabled_categories
            .contains(&ModerationCategory::Toxic)
        {
            return Ok(ModerationVerdict::Allow);
        }

        // Check blocked keywords first (highest priority)
        if let Some(reason) = self.check_blocked(content) {
            return Ok(ModerationVerdict::Block {
                category: ModerationCategory::Toxic,
                reason,
            });
        }

        // Check flagged keywords
        if let Some(reason) = self.check_flagged(content) {
            return Ok(ModerationVerdict::Flag {
                category: ModerationCategory::Toxic,
                reason,
            });
        }

        // Content is allowed
        Ok(ModerationVerdict::Allow)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_blocked_keyword() {
        let moderator = KeywordModerator::new()
            .add_blocked("spam")
            .add_blocked("scam");

        let policy = ContentPolicy::default();
        let result = moderator
            .moderate("This is spam content", &policy)
            .await
            .unwrap();
        assert!(result.is_blocked());
    }

    #[tokio::test]
    async fn test_flagged_keyword() {
        let moderator = KeywordModerator::new().add_flagged("warning");

        let policy = ContentPolicy::default();
        let result = moderator
            .moderate("This has a warning", &policy)
            .await
            .unwrap();
        assert!(!result.is_blocked());
        assert!(matches!(result, ModerationVerdict::Flag { .. }));
    }

    #[tokio::test]
    async fn test_allowed_content() {
        let moderator = KeywordModerator::new().add_blocked("spam");

        let policy = ContentPolicy::default();
        let result = moderator
            .moderate("This is clean content", &policy)
            .await
            .unwrap();
        assert!(result.is_allowed());
        assert!(matches!(result, ModerationVerdict::Allow));
    }

    #[tokio::test]
    async fn test_case_insensitive() {
        let moderator = KeywordModerator::new().add_blocked("SPAM");

        let policy = ContentPolicy::default();
        let result = moderator
            .moderate("This is spam content", &policy)
            .await
            .unwrap();
        assert!(result.is_blocked());
    }
}
