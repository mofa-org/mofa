//! Security Traits
//!
//! Core trait definitions for security governance components.
//!
//! These traits define the contracts that security implementations must follow.
//! Implementations are provided in `mofa-foundation`.

use crate::security::error::SecurityResult;
use crate::security::types::{DetectedPii, ModerationVerdict, RedactionStrategy};
use async_trait::async_trait;

/// Authorization result
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuthorizationResult {
    /// Permission granted
    Allowed,
    /// Permission denied with reason
    Denied(String),
}

impl AuthorizationResult {
    /// Check if permission is allowed
    pub fn is_allowed(&self) -> bool {
        matches!(self, AuthorizationResult::Allowed)
    }

    /// Check if permission is denied
    pub fn is_denied(&self) -> bool {
        matches!(self, AuthorizationResult::Denied(_))
    }

    /// Get denial reason if denied
    pub fn reason(&self) -> Option<&str> {
        match self {
            AuthorizationResult::Allowed => None,
            AuthorizationResult::Denied(reason) => Some(reason),
        }
    }
}

/// Authorizer trait for RBAC (Role-Based Access Control)
///
/// Checks if a subject (agent, user, etc.) has permission to perform
/// an action on a resource (tool, API endpoint, etc.).
#[async_trait]
pub trait Authorizer: Send + Sync {
    /// Check if a subject has permission to perform an action on a resource
    ///
    /// # Arguments
    /// * `subject` - The subject requesting access (e.g., agent ID, user ID)
    /// * `action` - The action being requested (e.g., "execute", "read", "write")
    /// * `resource` - The resource being accessed (e.g., "tool:delete_user", "api:users")
    ///
    /// # Returns
    /// `AuthorizationResult::Allowed` if permission is granted,
    /// `AuthorizationResult::Denied(reason)` if permission is denied.
    async fn check_permission(
        &self,
        subject: &str,
        action: &str,
        resource: &str,
    ) -> SecurityResult<AuthorizationResult>;
}

/// PII Detector trait
///
/// Detects sensitive data (PII) in text.
#[async_trait]
pub trait PiiDetector: Send + Sync {
    /// Detect sensitive data in the given text
    ///
    /// # Arguments
    /// * `text` - The text to scan for sensitive data
    ///
    /// # Returns
    /// Vector of detected PII with their positions and categories
    async fn detect(&self, text: &str) -> SecurityResult<Vec<DetectedPii>>;
}

/// PII Redactor trait
///
/// Redacts sensitive data from text using configurable strategies.
#[async_trait]
pub trait PiiRedactor: Send + Sync {
    /// Redact sensitive data from text
    ///
    /// # Arguments
    /// * `text` - The text to redact
    /// * `strategy` - The redaction strategy to use
    ///
    /// # Returns
    /// RedactionResult with the redacted text and metadata
    async fn redact(&self, text: &str, strategy: RedactionStrategy) -> SecurityResult<RedactionResult>;
}

/// Redaction result
#[derive(Debug, Clone)]
pub struct RedactionResult {
    /// The redacted text
    pub text: String,
    /// Number of redactions performed
    pub redaction_count: usize,
    /// Categories of data that were redacted
    pub redacted_categories: Vec<String>,
}

impl RedactionResult {
    /// Create a new redaction result
    pub fn new(text: String, redaction_count: usize, redacted_categories: Vec<String>) -> Self {
        Self {
            text,
            redaction_count,
            redacted_categories,
        }
    }
}

/// Content Moderator trait
///
/// Moderates content for harmful/toxic material.
#[async_trait]
pub trait ContentModerator: Send + Sync {
    /// Moderate content
    ///
    /// # Arguments
    /// * `content` - The content to moderate
    ///
    /// # Returns
    /// ModerationResult with verdict and optional reason
    async fn moderate(&self, content: &str) -> SecurityResult<ModerationResult>;
}

/// Moderation result
#[derive(Debug, Clone)]
pub struct ModerationResult {
    /// The moderation verdict
    pub verdict: ModerationVerdict,
    /// Optional additional metadata
    pub metadata: Option<serde_json::Value>,
}

impl ModerationResult {
    /// Create a new moderation result
    pub fn new(verdict: ModerationVerdict) -> Self {
        Self {
            verdict,
            metadata: None,
        }
    }

    /// Create a moderation result with metadata
    pub fn with_metadata(verdict: ModerationVerdict, metadata: serde_json::Value) -> Self {
        Self {
            verdict,
            metadata: Some(metadata),
        }
    }
}

/// Prompt Guard trait
///
/// Detects prompt injection attacks.
#[async_trait]
pub trait PromptGuard: Send + Sync {
    /// Check for prompt injection patterns
    ///
    /// # Arguments
    /// * `prompt` - The prompt to check
    ///
    /// # Returns
    /// InjectionCheckResult indicating if injection was detected
    async fn check_injection(&self, prompt: &str) -> SecurityResult<InjectionCheckResult>;
}

/// Injection check result
#[derive(Debug, Clone)]
pub struct InjectionCheckResult {
    /// Whether injection was detected
    pub is_suspicious: bool,
    /// Confidence score (0.0 to 1.0)
    pub confidence: f64,
    /// Detected pattern or reason
    pub reason: Option<String>,
}

impl InjectionCheckResult {
    /// Create a new injection check result
    pub fn new(is_suspicious: bool, confidence: f64, reason: Option<String>) -> Self {
        Self {
            is_suspicious,
            confidence,
            reason,
        }
    }

    /// Create a safe result (no injection detected)
    pub fn safe() -> Self {
        Self {
            is_suspicious: false,
            confidence: 0.0,
            reason: None,
        }
    }

    /// Create a suspicious result
    pub fn suspicious(confidence: f64, reason: String) -> Self {
        Self {
            is_suspicious: true,
            confidence,
            reason: Some(reason),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_authorization_result() {
        assert!(AuthorizationResult::Allowed.is_allowed());
        assert!(!AuthorizationResult::Allowed.is_denied());
        assert_eq!(AuthorizationResult::Allowed.reason(), None);

        let denied = AuthorizationResult::Denied("test".to_string());
        assert!(!denied.is_allowed());
        assert!(denied.is_denied());
        assert_eq!(denied.reason(), Some("test"));
    }

    #[test]
    fn test_injection_check_result() {
        let safe = InjectionCheckResult::safe();
        assert!(!safe.is_suspicious);
        assert_eq!(safe.confidence, 0.0);

        let suspicious = InjectionCheckResult::suspicious(0.9, "pattern".to_string());
        assert!(suspicious.is_suspicious);
        assert_eq!(suspicious.confidence, 0.9);
        assert_eq!(suspicious.reason, Some("pattern".to_string()));
    }
}
