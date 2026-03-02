//! Security Governance Module
//!
//! Provides kernel-level contracts for security governance including:
//! - **PII Redaction**: Detect and redact personally identifiable information
//! - **Content Moderation**: Filter harmful, toxic, or off-topic content
//! - **Prompt Guard**: Detect prompt injection attacks
//! - **Security Policy**: Composable policy configuration
//!
//! # Architecture
//!
//! This module defines traits (contracts) that follow the microkernel pattern.
//! Concrete implementations live in `mofa-foundation::security`.
//!
//! ```text
//! ┌──────────────────────────────────────────────────┐
//! │              mofa-kernel/security                 │
//! │  ┌────────────┐  ┌──────────────┐  ┌──────────┐ │
//! │  │ PiiDetector│  │ContentModera-│  │  Prompt  │ │
//! │  │ PiiRedactor│  │    tor       │  │  Guard   │ │
//! │  └────────────┘  └──────────────┘  └──────────┘ │
//! │  ┌──────────────────────────────────────────────┐│
//! │  │              SecurityPolicy                  ││
//! │  └──────────────────────────────────────────────┘│
//! └──────────────────────────────────────────────────┘
//!                        ▲ traits
//!                        │
//! ┌──────────────────────────────────────────────────┐
//! │            mofa-foundation/security              │
//! │  ┌────────────┐  ┌──────────────┐  ┌──────────┐ │
//! │  │RegexPii-   │  │ Keyword-     │  │  Regex-  │ │
//! │  │ Detector   │  │  Moderator   │  │  Prompt  │ │
//! │  │RegexPii-   │  │              │  │  Guard   │ │
//! │  │ Redactor   │  │              │  │          │ │
//! │  └────────────┘  └──────────────┘  └──────────┘ │
//! │  ┌──────────────────────────────────────────────┐│
//! │  │           SecurityMiddleware                 ││
//! │  └──────────────────────────────────────────────┘│
//! └──────────────────────────────────────────────────┘
//! ```

pub mod moderation;
pub mod policy;
pub mod redaction;
pub mod types;

// Re-export key types for convenience
pub use moderation::{ContentModerator, PromptGuard};
pub use policy::{PolicyBuilder, SecurityPolicy};
pub use redaction::{PiiDetector, PiiRedactor, RedactionAuditLog};
pub use types::{
    ContentPolicy, ModerationCategory, ModerationVerdict, RedactionMatch, RedactionResult,
    RedactionStrategy, SecurityError, SecurityResult, SensitiveDataCategory,
};
