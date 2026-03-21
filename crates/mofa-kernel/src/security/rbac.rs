//! RBAC (Role-Based Access Control) traits
//!
//! Kernel-level contracts for authorization and access control.

use super::types::SecurityResult;
use async_trait::async_trait;

/// Authorization result
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum AuthorizationResult {
    /// Permission granted
    Allowed,
    /// Permission denied with reason
    Denied(String),
}

impl AuthorizationResult {
    /// Check if permission is allowed
    #[must_use]
    pub fn is_allowed(&self) -> bool {
        matches!(self, AuthorizationResult::Allowed)
    }

    /// Check if permission is denied
    #[must_use]
    pub fn is_denied(&self) -> bool {
        matches!(self, AuthorizationResult::Denied(_))
    }

    /// Get denial reason if denied
    #[must_use]
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

    // Verify trait object safety
    fn _assert_authorizer_object_safe(_: &dyn Authorizer) {}
}
