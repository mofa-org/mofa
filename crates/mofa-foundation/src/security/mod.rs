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
