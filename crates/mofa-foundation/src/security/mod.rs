//! Security Governance Module — Foundation Implementations
//!
//! Concrete implementations of the security traits defined in `mofa-kernel::security`.
//!
//! - **`regex_pii`**: Regex-based PII detection and redaction
//! - **`keyword_moderator`**: Keyword-based content moderation and prompt guard

pub mod keyword_moderator;
pub mod regex_pii;

// Re-export main types for convenience
pub use keyword_moderator::{KeywordModerator, RegexPromptGuard};
pub use regex_pii::{RegexPiiDetector, RegexPiiRedactor};
