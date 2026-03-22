//! Content Policy
//!
//! Policy configuration for content moderation.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Content moderation categories
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[non_exhaustive]
pub enum ContentCategory {
    /// Hate speech
    Hate,
    /// Violence
    Violence,
    /// Self-harm
    SelfHarm,
    /// Sexual content
    Sexual,
    /// Harassment
    Harassment,
    /// Spam
    Spam,
    /// Misinformation
    Misinformation,
}

/// Content policy configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContentPolicy {
    /// Categories that should be blocked
    blocked_categories: Vec<ContentCategory>,
    /// Categories that should be flagged
    flagged_categories: Vec<ContentCategory>,
    /// Custom rules (category -> action)
    custom_rules: HashMap<String, String>,
}

impl ContentPolicy {
    /// Create a new content policy
    pub fn new() -> Self {
        Self {
            blocked_categories: Vec::new(),
            flagged_categories: Vec::new(),
            custom_rules: HashMap::new(),
        }
    }

    /// Add a blocked category
    pub fn add_blocked(mut self, category: ContentCategory) -> Self {
        self.blocked_categories.push(category);
        self
    }

    /// Add a flagged category
    pub fn add_flagged(mut self, category: ContentCategory) -> Self {
        self.flagged_categories.push(category);
        self
    }

    /// Check if a category is blocked
    pub fn is_blocked(&self, category: ContentCategory) -> bool {
        self.blocked_categories.contains(&category)
    }

    /// Check if a category is flagged
    pub fn is_flagged(&self, category: ContentCategory) -> bool {
        self.flagged_categories.contains(&category)
    }
}

impl Default for ContentPolicy {
    fn default() -> Self {
        Self::new()
    }
}
