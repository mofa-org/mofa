//! Security Implementations
//!
//! Concrete implementations of security governance traits defined in `mofa-runtime`.
//!
//! This module provides:
//! - RBAC (Role-Based Access Control) implementations
//! - PII detection and redaction implementations
//! - Content moderation implementations
//! - Prompt injection guard implementations

pub mod rbac;
pub mod pii;
pub mod moderation;
pub mod guard;

#[cfg(test)]
mod tests;

// Re-export commonly used types
pub use rbac::{DefaultAuthorizer, RbacPolicy, Role};
pub use pii::{RegexPiiDetector, RegexPiiRedactor};
pub use moderation::{ContentCategory, ContentPolicy, KeywordModerator};
pub use guard::RegexPromptGuard;
// Security Governance Module — Foundation Implementations
//
// Concrete implementations of the security traits defined in `mofa-kernel::security`.
//
// - **`regex_pii`**: Regex-based PII detection and redaction
// - **`keyword_moderator`**: Keyword-based content moderation and prompt guard

pub mod keyword_moderator;
pub mod regex_pii;

// Note: kernel-based implementations in `keyword_moderator` and `regex_pii`
// are available via their module paths:
// - `crate::security::keyword_moderator::*`
// - `crate::security::regex_pii::*`
// The top-level re-exports above remain the canonical public types for now.
