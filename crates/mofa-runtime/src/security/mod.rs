//! Security Governance Module
//!
//! This module provides security governance infrastructure for MoFA, including:
//! - RBAC (Role-Based Access Control) for tool permissions
//! - PII (Personally Identifiable Information) detection and redaction
//! - Content moderation for harmful/toxic content
//! - Prompt injection detection and prevention
//!
//! # Architecture
//!
//! This module follows the microkernel pattern:
//! - **Traits and types** are defined here in `mofa-runtime` (NOT in kernel)
//! - **Implementations** are provided in `mofa-foundation`
//! - **Kernel** only contains minimal `SecurityEvent` enum for event bus compatibility
//!
//! # Usage
//!
//! ```rust,ignore
//! use mofa_runtime::security::{SecurityService, SecurityConfig};
//!
//! let config = SecurityConfig::default()
//!     .with_rbac_enabled(true)
//!     .with_pii_redaction_enabled(true);
//!
//! let security_service = SecurityService::new(config)
//!     .with_authorizer(my_authorizer)
//!     .with_pii_redactor(my_redactor)
//!     .build();
//! ```

pub mod audit;
pub mod config;
pub mod error;
pub mod events;
pub mod traits;
pub mod types;

pub use audit::SecurityAuditLogger;
pub use config::SecurityConfig;
pub use error::{SecurityError, SecurityResult};
pub use events::SecurityEvent;
pub use traits::{Authorizer, AuthorizationResult, ContentModerator, ModerationResult, PiiDetector, PiiRedactor, PromptGuard, RedactionResult};
pub use types::{ModerationVerdict, RedactionStrategy, SensitiveDataCategory};

use std::sync::Arc;

/// Security Service Orchestrator
///
/// Coordinates all security components (RBAC, PII, moderation, prompt guard)
/// and provides a unified interface for security checks.
#[derive(Clone)]
pub struct SecurityService {
    /// Optional RBAC authorizer for permission checks
    pub(crate) authorizer: Option<Arc<dyn Authorizer>>,
    /// Optional PII detector for sensitive data detection
    pub(crate) pii_detector: Option<Arc<dyn PiiDetector>>,
    /// Optional PII redactor for data sanitization
    pub(crate) pii_redactor: Option<Arc<dyn PiiRedactor>>,
    /// Optional content moderator for harmful content filtering
    pub(crate) content_moderator: Option<Arc<dyn ContentModerator>>,
    /// Optional prompt guard for injection detection
    pub(crate) prompt_guard: Option<Arc<dyn PromptGuard>>,
    /// Security configuration
    pub(crate) config: SecurityConfig,
}

impl SecurityService {
    /// Create a new SecurityService with the given configuration
    pub fn new(config: SecurityConfig) -> Self {
        Self {
            authorizer: None,
            pii_detector: None,
            pii_redactor: None,
            content_moderator: None,
            prompt_guard: None,
            config,
        }
    }

    /// Set the authorizer for RBAC checks
    pub fn with_authorizer<A: Authorizer + 'static>(mut self, authorizer: A) -> Self {
        self.authorizer = Some(Arc::new(authorizer));
        self
    }

    /// Set the PII detector
    pub fn with_pii_detector<D: PiiDetector + 'static>(mut self, detector: D) -> Self {
        self.pii_detector = Some(Arc::new(detector));
        self
    }

    /// Set the PII redactor
    pub fn with_pii_redactor<R: PiiRedactor + 'static>(mut self, redactor: R) -> Self {
        self.pii_redactor = Some(Arc::new(redactor));
        self
    }

    /// Set the content moderator
    pub fn with_content_moderator<M: ContentModerator + 'static>(mut self, moderator: M) -> Self {
        self.content_moderator = Some(Arc::new(moderator));
        self
    }

    /// Set the prompt guard
    pub fn with_prompt_guard<G: PromptGuard + 'static>(mut self, guard: G) -> Self {
        self.prompt_guard = Some(Arc::new(guard));
        self
    }

    /// Check if RBAC is enabled and configured
    pub fn is_rbac_enabled(&self) -> bool {
        self.config.rbac_enabled && self.authorizer.is_some()
    }

    /// Check if PII redaction is enabled and configured
    pub fn is_pii_enabled(&self) -> bool {
        self.config.pii_redaction_enabled && (self.pii_detector.is_some() || self.pii_redactor.is_some())
    }

    /// Check if content moderation is enabled and configured
    pub fn is_moderation_enabled(&self) -> bool {
        self.config.content_moderation_enabled && self.content_moderator.is_some()
    }

    /// Check if prompt guard is enabled and configured
    pub fn is_prompt_guard_enabled(&self) -> bool {
        self.config.prompt_guard_enabled && self.prompt_guard.is_some()
    }

    /// Get a reference to the authorizer (if enabled)
    pub fn authorizer(&self) -> Option<&Arc<dyn Authorizer>> {
        self.authorizer.as_ref()
    }

    /// Get a reference to the PII detector (if enabled)
    pub fn pii_detector(&self) -> Option<&Arc<dyn PiiDetector>> {
        self.pii_detector.as_ref()
    }

    /// Get a reference to the PII redactor (if enabled)
    pub fn pii_redactor(&self) -> Option<&Arc<dyn PiiRedactor>> {
        self.pii_redactor.as_ref()
    }

    /// Get a reference to the content moderator (if enabled)
    pub fn content_moderator(&self) -> Option<&Arc<dyn ContentModerator>> {
        self.content_moderator.as_ref()
    }

    /// Get a reference to the prompt guard (if enabled)
    pub fn prompt_guard(&self) -> Option<&Arc<dyn PromptGuard>> {
        self.prompt_guard.as_ref()
    }

    /// Get a reference to the security configuration
    pub fn config(&self) -> &SecurityConfig {
        &self.config
    }
}

impl Default for SecurityService {
    fn default() -> Self {
        Self::new(SecurityConfig::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_security_service_creation() {
        let config = SecurityConfig::default()
            .with_rbac_enabled(true)
            .with_pii_redaction_enabled(true);
        let service = SecurityService::new(config);
        assert!(!service.is_rbac_enabled()); // No authorizer set yet
        assert!(!service.is_pii_enabled()); // No detector/redactor set yet
    }
}
