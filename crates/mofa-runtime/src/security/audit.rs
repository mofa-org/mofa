//! Security Audit Logging
//!
//! Helper functions for logging security events for compliance and monitoring.

use crate::security::events::SecurityEvent;
use tracing::{error, info, warn};

/// Audit logger for security events
pub struct SecurityAuditLogger;

impl SecurityAuditLogger {
    /// Log a security event
    pub fn log_event(event: &SecurityEvent) {
        match event {
            SecurityEvent::PermissionCheck {
                subject,
                action,
                resource,
                allowed,
                reason,
                ..
            } => {
                if *allowed {
                    info!(
                        subject = %subject,
                        action = %action,
                        resource = %resource,
                        "Security: Permission granted"
                    );
                } else {
                    warn!(
                        subject = %subject,
                        action = %action,
                        resource = %resource,
                        reason = %reason.as_ref().unwrap_or(&"unknown".to_string()),
                        "Security: Permission denied"
                    );
                }
            }
            SecurityEvent::PiiDetected {
                category, count, ..
            } => {
                warn!(
                    category = %category,
                    count = %count,
                    "Security: PII detected"
                );
            }
            SecurityEvent::PiiRedacted {
                count, categories, ..
            } => {
                info!(
                    count = %count,
                    categories = ?categories,
                    "Security: PII redacted"
                );
            }
            SecurityEvent::ContentModerated {
                verdict, reason, ..
            } => match verdict.as_str() {
                "block" => {
                    error!(
                        reason = %reason.as_ref().unwrap_or(&"unknown".to_string()),
                        "Security: Content blocked"
                    );
                }
                "flag" => {
                    warn!(
                        reason = %reason.as_ref().unwrap_or(&"unknown".to_string()),
                        "Security: Content flagged"
                    );
                }
                _ => {
                    info!("Security: Content allowed");
                }
            },
            SecurityEvent::PromptInjectionDetected {
                confidence,
                pattern,
                ..
            } => {
                error!(
                    confidence = %confidence,
                    pattern = %pattern,
                    "Security: Prompt injection detected"
                );
            }
        }
    }

    /// Log permission check
    pub fn log_permission_check(
        subject: &str,
        action: &str,
        resource: &str,
        allowed: bool,
        reason: Option<&str>,
    ) {
        let event = SecurityEvent::permission_check(
            subject.to_string(),
            action.to_string(),
            resource.to_string(),
            allowed,
            reason.map(|s| s.to_string()),
        );
        Self::log_event(&event);
    }

    /// Log PII detection
    pub fn log_pii_detection(category: &str, count: usize) {
        let event = SecurityEvent::pii_detected(category.to_string(), count);
        Self::log_event(&event);
    }

    /// Log PII redaction
    pub fn log_pii_redaction(count: usize, categories: Vec<String>) {
        let event = SecurityEvent::pii_redacted(count, categories);
        Self::log_event(&event);
    }

    /// Log content moderation
    pub fn log_content_moderation(verdict: &str, reason: Option<&str>) {
        let event =
            SecurityEvent::content_moderated(verdict.to_string(), reason.map(|s| s.to_string()));
        Self::log_event(&event);
    }

    /// Log prompt injection detection
    pub fn log_prompt_injection(confidence: f64, pattern: &str) {
        let event = SecurityEvent::prompt_injection_detected(confidence, pattern.to_string());
        Self::log_event(&event);
    }
}
