//! Default Authorizer Implementation
//!
//! Default implementation of the Authorizer trait using RBAC policy.

use crate::security::rbac::policy::RbacPolicy;
use async_trait::async_trait;
use mofa_kernel::security::{Authorizer, AuthorizationResult, SecurityResult};
use std::sync::Arc;
use tokio::sync::RwLock;

/// Default authorizer implementation using RBAC policy
pub struct DefaultAuthorizer {
    policy: Arc<RwLock<RbacPolicy>>,
}

impl DefaultAuthorizer {
    /// Create a new DefaultAuthorizer with the given policy
    pub fn new(policy: RbacPolicy) -> Self {
        Self {
            policy: Arc::new(RwLock::new(policy)),
        }
    }

    /// Create a new DefaultAuthorizer with an empty policy
    pub fn empty() -> Self {
        Self::new(RbacPolicy::new())
    }

    /// Update the RBAC policy
    pub async fn update_policy(&self, policy: RbacPolicy) {
        *self.policy.write().await = policy;
    }

    /// Get a reference to the policy (for read-only access)
    pub async fn policy(&self) -> tokio::sync::RwLockReadGuard<'_, RbacPolicy> {
        self.policy.read().await
    }

    /// Get a mutable reference to the policy
    pub async fn policy_mut(&self) -> tokio::sync::RwLockWriteGuard<'_, RbacPolicy> {
        self.policy.write().await
    }
}

#[async_trait]
impl Authorizer for DefaultAuthorizer {
    async fn check_permission(
        &self,
        subject: &str,
        action: &str,
        resource: &str,
    ) -> SecurityResult<AuthorizationResult> {
        // Format permission as "action:resource" (e.g., "execute:tool:delete_user")
        let permission = format!("{}:{}", action, resource);
        
        let policy = self.policy.read().await;
        let allowed = policy.check_permission(subject, &permission);
        
        if allowed {
            Ok(AuthorizationResult::Allowed)
        } else {
            Ok(AuthorizationResult::Denied(format!(
                "Subject '{}' does not have permission '{}'",
                subject, permission
            )))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::security::rbac::roles::Role;

    #[tokio::test]
    async fn test_default_authorizer() {
        let mut policy = RbacPolicy::new();
        
        let admin = Role::new("admin")
            .with_permission("execute:tool:delete");
        
        policy.add_role(admin);
        policy.assign_role("agent-1", "admin");
        
        let authorizer = DefaultAuthorizer::new(policy);
        
        // Check allowed permission
        let result = authorizer
            .check_permission("agent-1", "execute", "tool:delete")
            .await
            .unwrap();
        
        assert!(result.is_allowed());
        
        // Check denied permission
        let result = authorizer
            .check_permission("agent-1", "execute", "tool:create")
            .await
            .unwrap();
        
        assert!(result.is_denied());
    }
}
