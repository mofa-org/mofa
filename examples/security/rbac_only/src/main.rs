//! Minimal RBAC Example
//!
//! This example demonstrates a minimal setup for role-based access control (RBAC)
//! in MoFA agents. It shows how to:
//! - Define roles with permissions
//! - Assign roles to agents
//! - Check permissions before tool execution
//!
//! Run with: `cargo run --example rbac_only`

use mofa_foundation::security::{DefaultAuthorizer, RbacPolicy, Role};
use mofa_kernel::security::Authorizer;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("MoFA RBAC - Minimal Example\n");

    // ============================================================================
    // Step 1: Define Roles
    // ============================================================================
    println!("Step 1: Defining roles...");

    let admin_role = Role::new("admin")
        .with_permission("execute:tool:delete")
        .with_permission("execute:tool:create")
        .with_permission("execute:tool:read")
        .with_permission("execute:tool:update");

    let editor_role = Role::new("editor")
        .with_permission("execute:tool:read")
        .with_permission("execute:tool:update");

    let viewer_role = Role::new("viewer")
        .with_permission("execute:tool:read");

    println!("  Created roles: admin, editor, viewer");
    println!();

    // ============================================================================
    // Step 2: Create RBAC Policy
    // ============================================================================
    println!("Step 2: Creating RBAC policy...");

    let mut policy = RbacPolicy::new();
    policy.add_role(admin_role);
    policy.add_role(editor_role);
    policy.add_role(viewer_role);

    println!("  Added {} roles to policy", 3);
    println!();

    // ============================================================================
    // Step 3: Assign Roles to Agents
    // ============================================================================
    println!("Step 3: Assigning roles to agents...");

    policy.assign_role("agent-alice", "admin");
    policy.assign_role("agent-bob", "editor");
    policy.assign_role("agent-charlie", "viewer");

    println!("  Alice -> admin");
    println!("  Bob -> editor");
    println!("  Charlie -> viewer");
    println!();

    // ============================================================================
    // Step 4: Create Authorizer
    // ============================================================================
    println!("Step 4: Creating authorizer...");

    let authorizer = DefaultAuthorizer::new(policy);

    println!("  Authorizer ready");
    println!();

    // ============================================================================
    // Step 5: Test Permission Checks
    // ============================================================================
    println!("Step 5: Testing permission checks...\n");

    // Test cases: (agent, action, resource, expected)
    let test_cases = vec![
        ("agent-alice", "execute", "tool:delete", true, "Admin should be able to delete"),
        ("agent-alice", "execute", "tool:read", true, "Admin should be able to read"),
        ("agent-bob", "execute", "tool:update", true, "Editor should be able to update"),
        ("agent-bob", "execute", "tool:delete", false, "Editor should NOT be able to delete"),
        ("agent-charlie", "execute", "tool:read", true, "Viewer should be able to read"),
        ("agent-charlie", "execute", "tool:update", false, "Viewer should NOT be able to update"),
        ("agent-charlie", "execute", "tool:delete", false, "Viewer should NOT be able to delete"),
    ];

    for (agent, action, resource, expected_allowed, description) in test_cases {
        let result = authorizer
            .check_permission(agent, action, resource)
            .await?;

        let allowed = result.is_allowed();
        let status = if allowed == expected_allowed { "PASS" } else { "FAIL" };

        println!("  {}: {} -> {}:{} = {} ({})", 
                 status, agent, action, resource, allowed, description);
    }

    println!();

    // ============================================================================
    // Step 6: Role Inheritance Example
    // ============================================================================
    println!("Step 6: Demonstrating role inheritance...\n");

    let mut policy_with_inheritance = RbacPolicy::new();

    let base_role = Role::new("base")
        .with_permission("execute:tool:read");

    let manager_role = Role::new("manager")
        .with_permission("execute:tool:update")
        .with_parent_role("base");

    let director_role = Role::new("director")
        .with_permission("execute:tool:delete")
        .with_parent_role("manager");

    policy_with_inheritance.add_role(base_role);
    policy_with_inheritance.add_role(manager_role);
    policy_with_inheritance.add_role(director_role);
    policy_with_inheritance.assign_role("agent-director", "director");

    let authorizer_inheritance = DefaultAuthorizer::new(policy_with_inheritance);

    let inheritance_tests = vec![
        ("agent-director", "execute", "tool:read", "Director inherits read from base"),
        ("agent-director", "execute", "tool:update", "Director inherits update from manager"),
        ("agent-director", "execute", "tool:delete", "Director has delete permission"),
    ];

    for (agent, action, resource, description) in inheritance_tests {
        let result = authorizer_inheritance
            .check_permission(agent, action, resource)
            .await?;

        println!("  {} -> {}:{} = {} ({})", 
                 agent, action, resource, result.is_allowed(), description);
    }

    println!();
    println!("RBAC example completed successfully!");
    println!();
    println!("Next steps:");
    println!("  1. Integrate authorizer into your runtime");
    println!("  2. Check permissions before tool execution");
    println!("  3. Set up per-tenant policies for multi-tenant systems");

    Ok(())
}
