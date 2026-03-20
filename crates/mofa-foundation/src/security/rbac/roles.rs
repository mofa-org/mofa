//! Role Management
//!
//! Role definitions and role-to-permission mappings.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::collections::HashSet;

/// Role definition
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Role {
    /// Role name/identifier
    pub name: String,
    /// Permissions granted to this role
    pub permissions: HashSet<String>,
    /// Parent roles (for role inheritance)
    pub parent_roles: Vec<String>,
}

impl Role {
    /// Create a new role
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            permissions: HashSet::new(),
            parent_roles: Vec::new(),
        }
    }

    /// Add a permission to this role
    pub fn with_permission(mut self, permission: impl Into<String>) -> Self {
        self.permissions.insert(permission.into());
        self
    }

    /// Add multiple permissions
    pub fn with_permissions(mut self, permissions: impl IntoIterator<Item = String>) -> Self {
        self.permissions.extend(permissions);
        self
    }

    /// Add a parent role (for inheritance)
    pub fn with_parent_role(mut self, parent: impl Into<String>) -> Self {
        self.parent_roles.push(parent.into());
        self
    }

    /// Check if this role has a specific permission
    pub fn has_permission(&self, permission: &str) -> bool {
        self.permissions.contains(permission)
    }
}

/// Role registry for managing roles and their permissions
#[derive(Debug, Clone)]
pub struct RoleRegistry {
    roles: HashMap<String, Role>,
}

impl RoleRegistry {
    /// Create a new role registry
    pub fn new() -> Self {
        Self {
            roles: HashMap::new(),
        }
    }

    /// Register a role
    pub fn register_role(&mut self, role: Role) {
        self.roles.insert(role.name.clone(), role);
    }

    /// Get a role by name
    pub fn get_role(&self, name: &str) -> Option<&Role> {
        self.roles.get(name)
    }

    /// Get all permissions for a role (including inherited permissions)
    pub fn get_all_permissions(&self, role_name: &str) -> HashSet<String> {
        let mut permissions = HashSet::new();
        let mut visited = HashSet::new();
        self.collect_permissions(role_name, &mut permissions, &mut visited);
        permissions
    }

    /// Recursively collect permissions from role and its parents
    fn collect_permissions(
        &self,
        role_name: &str,
        permissions: &mut HashSet<String>,
        visited: &mut HashSet<String>,
    ) {
        if visited.contains(role_name) {
            return; // Avoid cycles
        }
        visited.insert(role_name.to_string());

        if let Some(role) = self.roles.get(role_name) {
            permissions.extend(role.permissions.iter().cloned());

            // Collect permissions from parent roles
            for parent_name in &role.parent_roles {
                self.collect_permissions(parent_name, permissions, visited);
            }
        }
    }

    /// Check if a role has a specific permission (including inherited)
    pub fn has_permission(&self, role_name: &str, permission: &str) -> bool {
        self.get_all_permissions(role_name).contains(permission)
    }
}

impl Default for RoleRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_role_creation() {
        let role = Role::new("admin")
            .with_permission("tool:delete")
            .with_permission("tool:create");
        
        assert!(role.has_permission("tool:delete"));
        assert!(role.has_permission("tool:create"));
        assert!(!role.has_permission("tool:read"));
    }

    #[test]
    fn test_role_registry() {
        let mut registry = RoleRegistry::new();
        
        let admin = Role::new("admin")
            .with_permission("tool:delete")
            .with_permission("tool:create");
        
        let user = Role::new("user")
            .with_permission("tool:read")
            .with_parent_role("admin"); // Inherit from admin
        
        registry.register_role(admin);
        registry.register_role(user);
        
        assert!(registry.has_permission("admin", "tool:delete"));
        assert!(registry.has_permission("user", "tool:read"));
        // User should inherit admin permissions
        assert!(registry.has_permission("user", "tool:delete"));
    }
}
