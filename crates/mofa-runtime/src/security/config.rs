//! Security Configuration
//!
//! Configuration for security governance features, including feature flags
//! and fail modes.

use super::types::SecurityFailMode;
use serde::{Deserialize, Serialize};

/// Security configuration with feature flags
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityConfig {
    /// Enable RBAC (Role-Based Access Control) for tool permissions
    #[serde(default = "default_true")]
    pub rbac_enabled: bool,
    /// Enable PII (Personally Identifiable Information) redaction
    #[serde(default = "default_true")]
    pub pii_redaction_enabled: bool,
    /// Enable content moderation for harmful/toxic content
    #[serde(default = "default_true")]
    pub content_moderation_enabled: bool,
    /// Enable prompt injection guard
    #[serde(default = "default_true")]
    pub prompt_guard_enabled: bool,
    /// Enable audit logging for security events
    #[serde(default = "default_true")]
    pub audit_logging_enabled: bool,
    /// Fail mode: how to handle security check failures
    #[serde(default)]
    pub fail_mode: SecurityFailMode,
}

fn default_true() -> bool {
    true
}

impl SecurityConfig {
    /// Create a new SecurityConfig with all features disabled
    pub fn new() -> Self {
        Self {
            rbac_enabled: false,
            pii_redaction_enabled: false,
            content_moderation_enabled: false,
            prompt_guard_enabled: false,
            audit_logging_enabled: false,
            fail_mode: SecurityFailMode::default(),
        }
    }

    /// Enable RBAC
    pub fn with_rbac_enabled(mut self, enabled: bool) -> Self {
        self.rbac_enabled = enabled;
        self
    }

    /// Enable PII redaction
    pub fn with_pii_redaction_enabled(mut self, enabled: bool) -> Self {
        self.pii_redaction_enabled = enabled;
        self
    }

    /// Enable content moderation
    pub fn with_content_moderation_enabled(mut self, enabled: bool) -> Self {
        self.content_moderation_enabled = enabled;
        self
    }

    /// Enable prompt guard
    pub fn with_prompt_guard_enabled(mut self, enabled: bool) -> Self {
        self.prompt_guard_enabled = enabled;
        self
    }

    /// Enable audit logging
    pub fn with_audit_logging_enabled(mut self, enabled: bool) -> Self {
        self.audit_logging_enabled = enabled;
        self
    }

    /// Set fail mode
    pub fn with_fail_mode(mut self, mode: SecurityFailMode) -> Self {
        self.fail_mode = mode;
        self
    }

    /// Create a permissive config (all features disabled, fail open)
    pub fn permissive() -> Self {
        Self::new().with_fail_mode(SecurityFailMode::FailOpen)
    }

    /// Create a strict config (all features enabled, fail closed)
    pub fn strict() -> Self {
        Self::default().with_fail_mode(SecurityFailMode::FailClosed)
    }
}

impl Default for SecurityConfig {
    fn default() -> Self {
        Self {
            rbac_enabled: true,
            pii_redaction_enabled: true,
            content_moderation_enabled: true,
            prompt_guard_enabled: true,
            audit_logging_enabled: true,
            fail_mode: SecurityFailMode::FailClosed,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_security_config_default() {
        let config = SecurityConfig::default();
        assert!(config.rbac_enabled);
        assert!(config.pii_redaction_enabled);
        assert!(config.content_moderation_enabled);
        assert!(config.prompt_guard_enabled);
        assert!(config.audit_logging_enabled);
        assert_eq!(config.fail_mode, SecurityFailMode::FailClosed);
    }

    #[test]
    fn test_security_config_new() {
        let config = SecurityConfig::new();
        assert!(!config.rbac_enabled);
        assert!(!config.pii_redaction_enabled);
        assert!(!config.content_moderation_enabled);
        assert!(!config.prompt_guard_enabled);
        assert!(!config.audit_logging_enabled);
    }

    #[test]
    fn test_security_config_builder() {
        let config = SecurityConfig::new()
            .with_rbac_enabled(true)
            .with_pii_redaction_enabled(false)
            .with_fail_mode(SecurityFailMode::FailOpen);

        assert!(config.rbac_enabled);
        assert!(!config.pii_redaction_enabled);
        assert_eq!(config.fail_mode, SecurityFailMode::FailOpen);
    }

    #[test]
    fn test_security_config_presets() {
        let permissive = SecurityConfig::permissive();
        assert!(!permissive.rbac_enabled);
        assert_eq!(permissive.fail_mode, SecurityFailMode::FailOpen);

        let strict = SecurityConfig::strict();
        assert!(strict.rbac_enabled);
        assert_eq!(strict.fail_mode, SecurityFailMode::FailClosed);
    }
}
