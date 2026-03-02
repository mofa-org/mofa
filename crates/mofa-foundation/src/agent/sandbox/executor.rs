//! Sandboxed Tool Executor
//!
//! Implements the kernel's `ToolSandbox` trait with timeout enforcement,
//! policy checks, audit logging, and output truncation.

use async_trait::async_trait;
use mofa_kernel::agent::components::tool::{
    SandboxCapability, SandboxResourceLimits, SandboxedResult, Tool, ToolInput, ToolResult,
    ToolSandbox,
};
use mofa_kernel::agent::context::AgentContext;
use mofa_kernel::agent::error::{AgentError, AgentResult};
use std::sync::Arc;
use std::time::Instant;
use tokio::time::Duration;

use super::config::SandboxConfig;
use super::policy::{DefaultSandboxPolicy, PolicyDecision, SandboxPolicy};

/// Sandboxed tool executor implementing the kernel's `ToolSandbox` trait.
///
/// Wraps tool execution with:
/// - Policy checks (capability evaluation before execution)
/// - Timeout enforcement (via `tokio::time::timeout`)
/// - Output truncation (if output exceeds configured limits)
/// - Audit logging (via `tracing`)
/// - Execution metrics capture
///
/// # Example
///
/// ```rust,ignore
/// use mofa_foundation::agent::sandbox::{SandboxConfig, SandboxedToolExecutor};
///
/// let sandbox = SandboxedToolExecutor::new(SandboxConfig::default());
/// let result = sandbox.execute_sandboxed(&tool, input, &ctx).await?;
/// ```
pub struct SandboxedToolExecutor {
    config: SandboxConfig,
    policy: Arc<dyn SandboxPolicy>,
}

impl SandboxedToolExecutor {
    /// Create a new sandboxed executor with the given configuration.
    ///
    /// Uses `DefaultSandboxPolicy` for capability evaluation.
    pub fn new(config: SandboxConfig) -> Self {
        Self {
            config,
            policy: Arc::new(DefaultSandboxPolicy),
        }
    }

    /// Create a sandboxed executor with a custom policy.
    pub fn with_policy(config: SandboxConfig, policy: Arc<dyn SandboxPolicy>) -> Self {
        Self { config, policy }
    }

    /// Get a reference to the sandbox configuration.
    pub fn config(&self) -> &SandboxConfig {
        &self.config
    }

    /// Truncate tool output if it exceeds the configured maximum size.
    fn maybe_truncate_output(&self, result: &mut ToolResult) -> bool {
        if let Some(max_bytes) = self.config.resource_limits.max_output_bytes {
            let output_str = result.to_string_output();
            if output_str.len() as u64 > max_bytes {
                let truncated = &output_str[..max_bytes as usize];
                result.output =
                    serde_json::Value::String(format!("{}... [truncated]", truncated));
                return true;
            }
        }
        false
    }

    /// Determine which capabilities a tool requires based on its metadata.
    fn required_capabilities(&self, tool: &dyn Tool) -> Vec<SandboxCapability> {
        let metadata = tool.metadata();
        let mut caps = Vec::new();

        if metadata.requires_network {
            caps.push(SandboxCapability::Network);
        }
        if metadata.requires_filesystem {
            caps.push(SandboxCapability::FileSystemRead);
        }

        caps
    }
}

#[async_trait]
impl ToolSandbox for SandboxedToolExecutor {
    async fn execute_sandboxed(
        &self,
        tool: &dyn Tool,
        input: ToolInput,
        ctx: &AgentContext,
    ) -> AgentResult<SandboxedResult> {
        let tool_name = tool.name().to_string();

        // Step 1: Policy check
        let decision = self.policy.evaluate(tool, &self.config);
        match &decision {
            PolicyDecision::Deny(reason) => {
                if self.config.audit_logging {
                    tracing::warn!(
                        tool = %tool_name,
                        reason = %reason,
                        "Sandbox policy denied tool execution"
                    );
                }
                return Err(AgentError::CapabilityDenied(reason.clone()));
            }
            PolicyDecision::AllowWithWarning(warning) => {
                if self.config.audit_logging {
                    tracing::warn!(
                        tool = %tool_name,
                        warning = %warning,
                        "Sandbox policy allowed tool with warning"
                    );
                }
            }
            PolicyDecision::Allow => {}
        }

        // Step 2: Capture capabilities used
        let capabilities_used = self.required_capabilities(tool);

        if self.config.audit_logging {
            tracing::info!(
                tool = %tool_name,
                capabilities = ?capabilities_used,
                timeout_ms = self.config.resource_limits.max_execution_time_ms,
                "Starting sandboxed tool execution"
            );
        }

        // Step 3: Execute with timeout
        let start = Instant::now();
        let timeout = Duration::from_millis(self.config.resource_limits.max_execution_time_ms);

        let exec_result = tokio::time::timeout(timeout, tool.execute(input, ctx)).await;

        let execution_time_ms = start.elapsed().as_millis() as u64;

        let mut result = match exec_result {
            Ok(tool_result) => tool_result,
            Err(_elapsed) => {
                if self.config.audit_logging {
                    tracing::error!(
                        tool = %tool_name,
                        timeout_ms = self.config.resource_limits.max_execution_time_ms,
                        "Tool execution timed out"
                    );
                }
                return Err(AgentError::ToolTimeout(
                    self.config.resource_limits.max_execution_time_ms,
                ));
            }
        };

        // Step 4: Truncate output if needed
        let output_truncated = self.maybe_truncate_output(&mut result);

        // Step 5: Audit log completion
        if self.config.audit_logging {
            tracing::info!(
                tool = %tool_name,
                duration_ms = execution_time_ms,
                success = result.success,
                output_truncated = output_truncated,
                "Sandboxed tool execution completed"
            );
        }

        Ok(SandboxedResult {
            result,
            execution_time_ms,
            output_truncated,
            capabilities_used,
        })
    }

    fn check_capabilities(&self, tool: &dyn Tool) -> AgentResult<()> {
        let decision = self.policy.evaluate(tool, &self.config);
        match decision {
            PolicyDecision::Deny(reason) => Err(AgentError::CapabilityDenied(reason)),
            _ => Ok(()),
        }
    }

    fn resource_limits(&self) -> &SandboxResourceLimits {
        &self.config.resource_limits
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mofa_kernel::agent::components::tool::ToolMetadata;

    /// A simple test tool that returns immediately
    struct EchoTool;

    #[async_trait]
    impl Tool for EchoTool {
        fn name(&self) -> &str {
            "echo"
        }
        fn description(&self) -> &str {
            "echoes input"
        }
        fn parameters_schema(&self) -> serde_json::Value {
            serde_json::json!({"type": "object"})
        }
        async fn execute(&self, input: ToolInput, _ctx: &AgentContext) -> ToolResult {
            let msg = input.get_str("message").unwrap_or("hello");
            ToolResult::success_text(format!("echo: {}", msg))
        }
    }

    /// A tool that sleeps to test timeout
    struct SlowTool {
        sleep_ms: u64,
    }

    #[async_trait]
    impl Tool for SlowTool {
        fn name(&self) -> &str {
            "slow_tool"
        }
        fn description(&self) -> &str {
            "sleeps then returns"
        }
        fn parameters_schema(&self) -> serde_json::Value {
            serde_json::json!({"type": "object"})
        }
        async fn execute(&self, _input: ToolInput, _ctx: &AgentContext) -> ToolResult {
            tokio::time::sleep(Duration::from_millis(self.sleep_ms)).await;
            ToolResult::success_text("done")
        }
    }

    /// A tool that requires network
    struct NetworkTool;

    #[async_trait]
    impl Tool for NetworkTool {
        fn name(&self) -> &str {
            "http_fetch"
        }
        fn description(&self) -> &str {
            "fetches HTTP"
        }
        fn parameters_schema(&self) -> serde_json::Value {
            serde_json::json!({"type": "object"})
        }
        async fn execute(&self, _input: ToolInput, _ctx: &AgentContext) -> ToolResult {
            ToolResult::success_text("fetched")
        }
        fn metadata(&self) -> ToolMetadata {
            ToolMetadata::default().needs_network()
        }
    }

    /// A tool that produces large output
    struct BigOutputTool {
        output_size: usize,
    }

    #[async_trait]
    impl Tool for BigOutputTool {
        fn name(&self) -> &str {
            "big_output"
        }
        fn description(&self) -> &str {
            "produces large output"
        }
        fn parameters_schema(&self) -> serde_json::Value {
            serde_json::json!({"type": "object"})
        }
        async fn execute(&self, _input: ToolInput, _ctx: &AgentContext) -> ToolResult {
            let output = "x".repeat(self.output_size);
            ToolResult::success_text(output)
        }
    }

    #[tokio::test]
    async fn test_sandboxed_execution_success() {
        let sandbox = SandboxedToolExecutor::new(SandboxConfig::default());
        let tool = EchoTool;
        let input = ToolInput::from_json(serde_json::json!({"message": "world"}));
        let ctx = AgentContext::new("test");

        let result = sandbox.execute_sandboxed(&tool, input, &ctx).await.unwrap();

        assert!(result.result.success);
        assert_eq!(result.result.as_text(), Some("echo: world"));
        assert!(!result.output_truncated);
        assert!(result.execution_time_ms < 1000); // Should be near-instant
    }

    #[tokio::test]
    async fn test_sandboxed_execution_timeout() {
        let config = SandboxConfig::default().with_timeout(50); // 50ms timeout
        let sandbox = SandboxedToolExecutor::new(config);
        let tool = SlowTool { sleep_ms: 5000 }; // 5 second sleep
        let input = ToolInput::from_json(serde_json::json!({}));
        let ctx = AgentContext::new("test");

        let result = sandbox.execute_sandboxed(&tool, input, &ctx).await;

        assert!(result.is_err());
        match result.unwrap_err() {
            AgentError::ToolTimeout(ms) => assert_eq!(ms, 50),
            err => panic!("Expected ToolTimeout, got: {:?}", err),
        }
    }

    #[tokio::test]
    async fn test_sandbox_denies_network_tool() {
        let config = SandboxConfig::restrictive(); // no network
        let sandbox = SandboxedToolExecutor::new(config);
        let tool = NetworkTool;
        let input = ToolInput::from_json(serde_json::json!({}));
        let ctx = AgentContext::new("test");

        let result = sandbox.execute_sandboxed(&tool, input, &ctx).await;

        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), AgentError::CapabilityDenied(_)));
    }

    #[tokio::test]
    async fn test_sandbox_allows_network_tool_when_permitted() {
        let config = SandboxConfig::default(); // network allowed
        let sandbox = SandboxedToolExecutor::new(config);
        let tool = NetworkTool;
        let input = ToolInput::from_json(serde_json::json!({}));
        let ctx = AgentContext::new("test");

        let result = sandbox.execute_sandboxed(&tool, input, &ctx).await.unwrap();
        assert!(result.result.success);
    }

    #[tokio::test]
    async fn test_sandbox_output_truncation() {
        let config = SandboxConfig::default().with_max_output(100); // 100 bytes max
        let sandbox = SandboxedToolExecutor::new(config);
        let tool = BigOutputTool { output_size: 1000 }; // 1000 byte output
        let input = ToolInput::from_json(serde_json::json!({}));
        let ctx = AgentContext::new("test");

        let result = sandbox.execute_sandboxed(&tool, input, &ctx).await.unwrap();

        assert!(result.output_truncated);
        let output_text = result.result.to_string_output();
        assert!(output_text.contains("[truncated]"));
    }

    #[tokio::test]
    async fn test_check_capabilities() {
        let config = SandboxConfig::restrictive();
        let sandbox = SandboxedToolExecutor::new(config);

        let echo = EchoTool;
        assert!(sandbox.check_capabilities(&echo).is_ok());

        let network = NetworkTool;
        assert!(sandbox.check_capabilities(&network).is_err());
    }

    #[test]
    fn test_resource_limits_accessor() {
        let config = SandboxConfig::default().with_timeout(42_000);
        let sandbox = SandboxedToolExecutor::new(config);

        assert_eq!(sandbox.resource_limits().max_execution_time_ms, 42_000);
    }
}
