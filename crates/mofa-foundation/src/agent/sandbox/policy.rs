//! Sandbox Policy Engine
//!
//! Decides whether a tool is allowed to execute based on its metadata
//! and the sandbox configuration.

use mofa_kernel::agent::components::tool::{SandboxCapability, Tool};

use super::config::SandboxConfig;

/// Result of a policy evaluation.
#[derive(Debug, Clone, PartialEq)]
pub enum PolicyDecision {
    /// Tool is allowed to execute
    Allow,
    /// Tool is denied with a reason
    Deny(String),
    /// Tool is allowed but with a warning
    AllowWithWarning(String),
}

impl PolicyDecision {
    /// Returns true if the tool is allowed to execute (Allow or AllowWithWarning).
    pub fn is_allowed(&self) -> bool {
        matches!(
            self,
            PolicyDecision::Allow | PolicyDecision::AllowWithWarning(_)
        )
    }
}

/// Trait for sandbox policy implementations.
///
/// Policies evaluate a tool's metadata against the sandbox configuration
/// to decide if it should be allowed to execute.
pub trait SandboxPolicy: Send + Sync {
    /// Evaluate whether a tool should be allowed to execute.
    fn evaluate(&self, tool: &dyn Tool, config: &SandboxConfig) -> PolicyDecision;
}

/// Default sandbox policy implementation.
///
/// Maps `ToolMetadata` flags to `SandboxCapability` checks:
/// - `requires_network` → checks `SandboxCapability::Network`
/// - `requires_filesystem` → checks `SandboxCapability::FileSystemRead`
/// - `is_dangerous` → checks `config.allow_dangerous_tools`
pub struct DefaultSandboxPolicy;

impl SandboxPolicy for DefaultSandboxPolicy {
    fn evaluate(&self, tool: &dyn Tool, config: &SandboxConfig) -> PolicyDecision {
        let metadata = tool.metadata();

        // Check dangerous tools
        if metadata.is_dangerous && !config.allow_dangerous_tools {
            return PolicyDecision::Deny(format!(
                "Tool '{}' is marked as dangerous and dangerous tools are not allowed",
                tool.name()
            ));
        }

        // Check network capability
        if metadata.requires_network && !config.is_capability_allowed(&SandboxCapability::Network) {
            return PolicyDecision::Deny(format!(
                "Tool '{}' requires network access but it is not allowed",
                tool.name()
            ));
        }

        // Check filesystem capability
        if metadata.requires_filesystem
            && !config.is_capability_allowed(&SandboxCapability::FileSystemRead)
        {
            return PolicyDecision::Deny(format!(
                "Tool '{}' requires filesystem access but it is not allowed",
                tool.name()
            ));
        }

        // Warn about dangerous tools that are explicitly allowed
        if metadata.is_dangerous && config.allow_dangerous_tools {
            return PolicyDecision::AllowWithWarning(format!(
                "Tool '{}' is marked as dangerous — proceeding with caution",
                tool.name()
            ));
        }

        PolicyDecision::Allow
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use mofa_kernel::agent::components::tool::{ToolInput, ToolMetadata, ToolResult};
    use mofa_kernel::agent::context::AgentContext;

    /// Test tool with configurable metadata
    struct TestTool {
        name: String,
        metadata: ToolMetadata,
    }

    impl TestTool {
        fn safe(name: &str) -> Self {
            Self {
                name: name.to_string(),
                metadata: ToolMetadata::default(),
            }
        }

        fn dangerous(name: &str) -> Self {
            Self {
                name: name.to_string(),
                metadata: ToolMetadata::default().dangerous(),
            }
        }

        fn needs_network(name: &str) -> Self {
            Self {
                name: name.to_string(),
                metadata: ToolMetadata::default().needs_network(),
            }
        }

        fn needs_filesystem(name: &str) -> Self {
            Self {
                name: name.to_string(),
                metadata: ToolMetadata::default().needs_filesystem(),
            }
        }
    }

    #[async_trait]
    impl Tool for TestTool {
        fn name(&self) -> &str {
            &self.name
        }
        fn description(&self) -> &str {
            "test tool"
        }
        fn parameters_schema(&self) -> serde_json::Value {
            serde_json::json!({"type": "object"})
        }
        async fn execute(&self, _input: ToolInput, _ctx: &AgentContext) -> ToolResult {
            ToolResult::success_text("ok")
        }
        fn metadata(&self) -> ToolMetadata {
            self.metadata.clone()
        }
    }

    #[test]
    fn test_policy_allows_safe_tool() {
        let policy = DefaultSandboxPolicy;
        let tool = TestTool::safe("calculator");
        let config = SandboxConfig::default();

        let decision = policy.evaluate(&tool, &config);
        assert_eq!(decision, PolicyDecision::Allow);
    }

    #[test]
    fn test_policy_denies_dangerous_tool() {
        let policy = DefaultSandboxPolicy;
        let tool = TestTool::dangerous("rm_rf");
        let config = SandboxConfig::default(); // allow_dangerous_tools = false

        let decision = policy.evaluate(&tool, &config);
        assert!(matches!(decision, PolicyDecision::Deny(_)));
    }

    #[test]
    fn test_policy_warns_dangerous_when_allowed() {
        let policy = DefaultSandboxPolicy;
        let tool = TestTool::dangerous("rm_rf");
        let config = SandboxConfig::default().with_allow_dangerous(true);

        let decision = policy.evaluate(&tool, &config);
        assert!(matches!(decision, PolicyDecision::AllowWithWarning(_)));
        assert!(decision.is_allowed());
    }

    #[test]
    fn test_policy_denies_network_tool() {
        let policy = DefaultSandboxPolicy;
        let tool = TestTool::needs_network("http_client");
        let config = SandboxConfig::restrictive(); // no capabilities allowed

        let decision = policy.evaluate(&tool, &config);
        assert!(matches!(decision, PolicyDecision::Deny(_)));
    }

    #[test]
    fn test_policy_allows_network_tool_when_permitted() {
        let policy = DefaultSandboxPolicy;
        let tool = TestTool::needs_network("http_client");
        let config = SandboxConfig::default(); // network allowed by default

        let decision = policy.evaluate(&tool, &config);
        assert_eq!(decision, PolicyDecision::Allow);
    }

    #[test]
    fn test_policy_denies_filesystem_tool() {
        let policy = DefaultSandboxPolicy;
        let tool = TestTool::needs_filesystem("file_reader");
        let config = SandboxConfig::restrictive();

        let decision = policy.evaluate(&tool, &config);
        assert!(matches!(decision, PolicyDecision::Deny(_)));
    }

    #[test]
    fn test_policy_decision_is_allowed() {
        assert!(PolicyDecision::Allow.is_allowed());
        assert!(PolicyDecision::AllowWithWarning("test".to_string()).is_allowed());
        assert!(!PolicyDecision::Deny("nope".to_string()).is_allowed());
    }
}
