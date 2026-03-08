//! Sandbox Configuration
//!
//! Configurable settings for tool execution sandboxing.

use mofa_kernel::agent::components::tool::{SandboxCapability, SandboxResourceLimits};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

/// Configuration for the tool execution sandbox.
///
/// Controls which capabilities are allowed/denied and what resource limits
/// are enforced during tool execution.
///
/// # Example
///
/// ```rust,ignore
/// use mofa_foundation::agent::sandbox::SandboxConfig;
/// use mofa_kernel::agent::components::tool::SandboxCapability;
///
/// let config = SandboxConfig::restrictive()
///     .with_timeout(5000)
///     .allow_capability(SandboxCapability::Network);
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SandboxConfig {
    /// Resource limits for sandboxed execution
    pub resource_limits: SandboxResourceLimits,
    /// Capabilities that are explicitly allowed
    pub allowed_capabilities: HashSet<SandboxCapability>,
    /// Capabilities that are explicitly denied (takes precedence over allowed)
    pub denied_capabilities: HashSet<SandboxCapability>,
    /// Whether to log all sandbox events for audit
    pub audit_logging: bool,
    /// Whether to allow tools marked as dangerous
    pub allow_dangerous_tools: bool,
}

impl Default for SandboxConfig {
    fn default() -> Self {
        // Default: moderate security — allows most capabilities, has reasonable limits
        let mut allowed = HashSet::new();
        allowed.insert(SandboxCapability::Network);
        allowed.insert(SandboxCapability::FileSystemRead);
        allowed.insert(SandboxCapability::EnvAccess);

        Self {
            resource_limits: SandboxResourceLimits::default(),
            allowed_capabilities: allowed,
            denied_capabilities: HashSet::new(),
            audit_logging: true,
            allow_dangerous_tools: false,
        }
    }
}

impl SandboxConfig {
    /// Create a permissive sandbox configuration.
    ///
    /// Allows all capabilities with generous resource limits.
    /// Use for trusted tool environments.
    pub fn permissive() -> Self {
        let mut allowed = HashSet::new();
        allowed.insert(SandboxCapability::Network);
        allowed.insert(SandboxCapability::FileSystemRead);
        allowed.insert(SandboxCapability::FileSystemWrite);
        allowed.insert(SandboxCapability::ProcessExec);
        allowed.insert(SandboxCapability::EnvAccess);
        allowed.insert(SandboxCapability::UnlimitedTime);

        Self {
            resource_limits: SandboxResourceLimits {
                max_execution_time_ms: 300_000, // 5 minutes
                max_memory_bytes: None,         // No memory limit
                max_output_bytes: None,         // No output limit
            },
            allowed_capabilities: allowed,
            denied_capabilities: HashSet::new(),
            audit_logging: false,
            allow_dangerous_tools: true,
        }
    }

    /// Create a restrictive sandbox configuration.
    ///
    /// Denies most capabilities with tight resource limits.
    /// Use for untrusted or third-party tools.
    pub fn restrictive() -> Self {
        Self {
            resource_limits: SandboxResourceLimits {
                max_execution_time_ms: 5_000,             // 5 seconds
                max_memory_bytes: Some(10 * 1024 * 1024), // 10 MB
                max_output_bytes: Some(256 * 1024),       // 256 KB
            },
            allowed_capabilities: HashSet::new(), // Nothing allowed by default
            denied_capabilities: HashSet::new(),
            audit_logging: true,
            allow_dangerous_tools: false,
        }
    }

    /// Set the maximum execution timeout in milliseconds.
    pub fn with_timeout(mut self, timeout_ms: u64) -> Self {
        self.resource_limits.max_execution_time_ms = timeout_ms;
        self
    }

    /// Set the maximum memory usage in bytes.
    pub fn with_max_memory(mut self, bytes: u64) -> Self {
        self.resource_limits.max_memory_bytes = Some(bytes);
        self
    }

    /// Set the maximum output size in bytes.
    pub fn with_max_output(mut self, bytes: u64) -> Self {
        self.resource_limits.max_output_bytes = Some(bytes);
        self
    }

    /// Allow a specific capability.
    pub fn allow_capability(mut self, cap: SandboxCapability) -> Self {
        self.denied_capabilities.remove(&cap);
        self.allowed_capabilities.insert(cap);
        self
    }

    /// Deny a specific capability (takes precedence over allow).
    pub fn deny_capability(mut self, cap: SandboxCapability) -> Self {
        self.allowed_capabilities.remove(&cap);
        self.denied_capabilities.insert(cap);
        self
    }

    /// Enable or disable audit logging.
    pub fn with_audit_logging(mut self, enabled: bool) -> Self {
        self.audit_logging = enabled;
        self
    }

    /// Allow or deny dangerous tools.
    pub fn with_allow_dangerous(mut self, allowed: bool) -> Self {
        self.allow_dangerous_tools = allowed;
        self
    }

    /// Check if a capability is allowed by this configuration.
    pub fn is_capability_allowed(&self, cap: &SandboxCapability) -> bool {
        if self.denied_capabilities.contains(cap) {
            return false;
        }
        self.allowed_capabilities.contains(cap)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = SandboxConfig::default();
        assert!(config.is_capability_allowed(&SandboxCapability::Network));
        assert!(config.is_capability_allowed(&SandboxCapability::FileSystemRead));
        assert!(!config.is_capability_allowed(&SandboxCapability::FileSystemWrite));
        assert!(!config.is_capability_allowed(&SandboxCapability::ProcessExec));
        assert!(config.audit_logging);
        assert!(!config.allow_dangerous_tools);
    }

    #[test]
    fn test_permissive_config() {
        let config = SandboxConfig::permissive();
        assert!(config.is_capability_allowed(&SandboxCapability::Network));
        assert!(config.is_capability_allowed(&SandboxCapability::FileSystemWrite));
        assert!(config.is_capability_allowed(&SandboxCapability::ProcessExec));
        assert!(config.allow_dangerous_tools);
        assert!(!config.audit_logging);
    }

    #[test]
    fn test_restrictive_config() {
        let config = SandboxConfig::restrictive();
        assert!(!config.is_capability_allowed(&SandboxCapability::Network));
        assert!(!config.is_capability_allowed(&SandboxCapability::FileSystemRead));
        assert_eq!(config.resource_limits.max_execution_time_ms, 5_000);
        assert!(!config.allow_dangerous_tools);
    }

    #[test]
    fn test_deny_overrides_allow() {
        let config = SandboxConfig::default()
            .allow_capability(SandboxCapability::Network)
            .deny_capability(SandboxCapability::Network);
        assert!(!config.is_capability_allowed(&SandboxCapability::Network));
    }

    #[test]
    fn test_builder_methods() {
        let config = SandboxConfig::restrictive()
            .with_timeout(10_000)
            .with_max_memory(100 * 1024 * 1024)
            .with_max_output(512 * 1024)
            .allow_capability(SandboxCapability::Network)
            .with_audit_logging(false)
            .with_allow_dangerous(true);

        assert_eq!(config.resource_limits.max_execution_time_ms, 10_000);
        assert_eq!(
            config.resource_limits.max_memory_bytes,
            Some(100 * 1024 * 1024)
        );
        assert_eq!(config.resource_limits.max_output_bytes, Some(512 * 1024));
        assert!(config.is_capability_allowed(&SandboxCapability::Network));
        assert!(!config.audit_logging);
        assert!(config.allow_dangerous_tools);
    }
}
