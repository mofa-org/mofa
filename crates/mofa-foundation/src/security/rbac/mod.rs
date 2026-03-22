//! RBAC (Role-Based Access Control) Implementation
//!
//! Provides role-based permission checking for tool access and agent actions.

pub mod authorizer;
pub mod policy;
pub mod roles;

pub use authorizer::DefaultAuthorizer;
pub use policy::RbacPolicy;
pub use roles::Role;
