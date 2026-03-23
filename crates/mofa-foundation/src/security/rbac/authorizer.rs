//! Default Authorizer Implementation
//!
//! Default implementation of the Authorizer trait using RBAC policy.

use crate::security::rbac::policy::RbacPolicy;
use async_trait::async_trait;
use mofa_kernel::security::{AuthorizationResult, Authorizer, SecurityResult};
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
    use std::sync::Arc;

    #[tokio::test]
    async fn test_default_authorizer() {
        let mut policy = RbacPolicy::new();

        let admin = Role::new("admin").with_permission("execute:tool:delete");

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

    #[tokio::test]
    async fn test_empty_authorizer_denies_with_reason() {
        let authorizer = DefaultAuthorizer::empty();

        let result = authorizer
            .check_permission("agent-1", "execute", "tool:delete")
            .await
            .unwrap();

        match result {
            AuthorizationResult::Denied(reason) => {
                assert!(reason.contains("agent-1"));
                assert!(reason.contains("execute:tool:delete"));
            }
            AuthorizationResult::Allowed => panic!("expected denied result"),
            _ => panic!("expected denied result"),
        }
    }

    #[tokio::test]
    async fn test_update_policy_changes_result() {
        let authorizer = DefaultAuthorizer::empty();

        let initial = authorizer
            .check_permission("agent-1", "execute", "tool:delete")
            .await
            .unwrap();
        assert!(initial.is_denied());

        let mut new_policy = RbacPolicy::new();
        let admin = Role::new("admin").with_permission("execute:tool:delete");
        new_policy.add_role(admin);
        new_policy.assign_role("agent-1", "admin");
        authorizer.update_policy(new_policy).await;

        let updated = authorizer
            .check_permission("agent-1", "execute", "tool:delete")
            .await
            .unwrap();
        assert!(updated.is_allowed());
    }

    #[tokio::test]
    async fn test_policy_mut_allows_dynamic_assignment() {
        let authorizer = DefaultAuthorizer::empty();
        {
            let mut policy = authorizer.policy_mut().await;
            let writer = Role::new("writer").with_permission("write:doc");
            policy.add_role(writer);
            policy.assign_role("agent-2", "writer");
        }

        let result = authorizer
            .check_permission("agent-2", "write", "doc")
            .await
            .unwrap();
        assert!(result.is_allowed());
    }

    #[tokio::test]
    async fn test_default_role_is_honored() {
        let mut policy = RbacPolicy::new().with_default_role("guest");
        let guest = Role::new("guest").with_permission("read:public");
        policy.add_role(guest);

        let authorizer = DefaultAuthorizer::new(policy);
        let result = authorizer
            .check_permission("unknown-agent", "read", "public")
            .await
            .unwrap();

        assert!(result.is_allowed());
    }

    #[tokio::test]
    async fn test_permission_formatting_action_resource() {
        let mut policy = RbacPolicy::new();
        let admin = Role::new("admin").with_permission("execute:tool:admin:rotate");
        policy.add_role(admin);
        policy.assign_role("agent-1", "admin");

        let authorizer = DefaultAuthorizer::new(policy);
        let result = authorizer
            .check_permission("agent-1", "execute", "tool:admin:rotate")
            .await
            .unwrap();

        assert!(result.is_allowed());
    }

    #[tokio::test]
    async fn test_concurrent_checks_for_mixed_subjects() {
        let mut policy = RbacPolicy::new();
        let admin = Role::new("admin").with_permission("execute:tool:delete");
        policy.add_role(admin);
        policy.assign_role("agent-1", "admin");

        let authorizer = Arc::new(DefaultAuthorizer::new(policy));
        let mut tasks = Vec::new();

        for subject in ["agent-1", "agent-2", "agent-1", "agent-2"] {
            let authorizer = Arc::clone(&authorizer);
            let subject = subject.to_string();
            tasks.push(tokio::spawn(async move {
                authorizer
                    .check_permission(&subject, "execute", "tool:delete")
                    .await
                    .unwrap()
            }));
        }

        let mut allowed = 0;
        let mut denied = 0;
        for task in tasks {
            match task.await.unwrap() {
                AuthorizationResult::Allowed => allowed += 1,
                AuthorizationResult::Denied(_) => denied += 1,
                _ => {}
            }
        }

        assert_eq!(allowed, 2);
        assert_eq!(denied, 2);
    }
}
