//! Null (passthrough) sandbox backend.
//!
//! `NullSandbox` implements [`ToolSandbox`] for trusted tools. It runs the
//! wrapped tool without entering any isolate — policy is consulted only at
//! the `precheck` boundary. Appropriate for first-party, audited tools
//! where isolation cost is not justified.
//!
//! Declared tier: [`SandboxTier::None`].

use std::sync::Arc;
use std::time::Instant;

use async_trait::async_trait;
use mofa_kernel::agent::components::sandbox::{
    SandboxError, SandboxExecutionStats, SandboxPolicy, SandboxRequest, SandboxResponse,
    SandboxResult, SandboxTier, ToolSandbox,
};
use mofa_kernel::agent::components::tool::{Tool, ToolInput};
use mofa_kernel::agent::context::AgentContext;

/// Passthrough sandbox — runs the wrapped tool directly in the host process
/// without any isolation beyond the kernel policy precheck.
pub struct NullSandbox {
    name: String,
    policy: SandboxPolicy,
    tool: Arc<
        dyn Tool<serde_json::Value, serde_json::Value> + Send + Sync + 'static,
    >,
    context: Arc<AgentContext>,
}

impl NullSandbox {
    /// Wrap a tool in a null sandbox. The policy is still validated and
    /// prechecked on every call, but the tool runs in the host process with
    /// no resource enforcement.
    pub fn new(
        name: impl Into<String>,
        policy: SandboxPolicy,
        tool: Arc<
            dyn Tool<serde_json::Value, serde_json::Value> + Send + Sync + 'static,
        >,
        context: Arc<AgentContext>,
    ) -> SandboxResult<Self> {
        policy.validate()?;
        Ok(Self {
            name: name.into(),
            policy,
            tool,
            context,
        })
    }
}

#[async_trait]
impl ToolSandbox for NullSandbox {
    fn name(&self) -> &str {
        &self.name
    }

    fn tier(&self) -> SandboxTier {
        SandboxTier::None
    }

    fn policy(&self) -> &SandboxPolicy {
        &self.policy
    }

    async fn execute(&self, req: SandboxRequest) -> SandboxResult<SandboxResponse> {
        self.precheck(&req)?;

        let input_bytes = req.arguments.to_string().len() as u64;
        let start = Instant::now();

        let tool_input = ToolInput::from_json(req.arguments);
        let tool_result = self.tool.execute(tool_input, &self.context).await;

        let elapsed_ms = start.elapsed().as_millis() as u64;

        if !tool_result.success {
            return Err(SandboxError::BackendFailure(
                tool_result
                    .error
                    .unwrap_or_else(|| "tool reported failure with no error message".into()),
            ));
        }

        let output_bytes = tool_result.output.to_string().len() as u64;

        Ok(SandboxResponse {
            output: tool_result.output,
            stats: SandboxExecutionStats {
                wall_time_ms: Some(elapsed_ms),
                cpu_time_ms: None,
                peak_memory_bytes: None,
                input_bytes: Some(input_bytes),
                output_bytes: Some(output_bytes),
                denials: 0,
            },
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use mofa_kernel::agent::components::sandbox::{SandboxCapability, SandboxPolicy};
    use mofa_kernel::agent::components::tool::{ToolMetadata, ToolResult};

    struct EchoTool;

    #[async_trait]
    impl Tool<serde_json::Value, serde_json::Value> for EchoTool {
        fn name(&self) -> &str {
            "echo"
        }
        fn description(&self) -> &str {
            "Echoes arguments back"
        }
        fn parameters_schema(&self) -> serde_json::Value {
            serde_json::json!({"type": "object"})
        }
        async fn execute(
            &self,
            input: ToolInput<serde_json::Value>,
            _ctx: &AgentContext,
        ) -> ToolResult<serde_json::Value> {
            ToolResult::success(input.arguments)
        }
        fn metadata(&self) -> ToolMetadata {
            ToolMetadata::default()
        }
    }

    struct FailingTool;

    #[async_trait]
    impl Tool<serde_json::Value, serde_json::Value> for FailingTool {
        fn name(&self) -> &str {
            "failing"
        }
        fn description(&self) -> &str {
            "Always fails"
        }
        fn parameters_schema(&self) -> serde_json::Value {
            serde_json::json!({"type": "object"})
        }
        async fn execute(
            &self,
            _input: ToolInput<serde_json::Value>,
            _ctx: &AgentContext,
        ) -> ToolResult<serde_json::Value> {
            ToolResult::failure("intentional failure")
        }
        fn metadata(&self) -> ToolMetadata {
            ToolMetadata::default()
        }
    }

    fn default_context() -> Arc<AgentContext> {
        Arc::new(AgentContext::new("test-agent"))
    }

    #[tokio::test]
    async fn null_sandbox_runs_tool_and_returns_output() {
        let sb = NullSandbox::new(
            "null-echo",
            SandboxPolicy::builder()
                .allow(SandboxCapability::Compute)
                .build()
                .unwrap(),
            Arc::new(EchoTool),
            default_context(),
        )
        .unwrap();

        let req = SandboxRequest::new("echo", serde_json::json!({"msg": "hi"}))
            .with_capability(SandboxCapability::Compute);
        let resp = sb.execute(req).await.unwrap();
        assert_eq!(resp.output, serde_json::json!({"msg": "hi"}));
        assert!(resp.stats.wall_time_ms.is_some());
    }

    #[tokio::test]
    async fn null_sandbox_reports_tool_failure_as_backend_failure() {
        let sb = NullSandbox::new(
            "null-fail",
            SandboxPolicy::builder()
                .allow(SandboxCapability::Compute)
                .build()
                .unwrap(),
            Arc::new(FailingTool),
            default_context(),
        )
        .unwrap();

        let req = SandboxRequest::new("failing", serde_json::json!({}))
            .with_capability(SandboxCapability::Compute);
        let err = sb.execute(req).await.unwrap_err();
        assert!(err.is_backend_failure());
    }

    #[tokio::test]
    async fn null_sandbox_denies_undeclared_capability() {
        let sb = NullSandbox::new(
            "null-deny",
            SandboxPolicy::denied_by_default(),
            Arc::new(EchoTool),
            default_context(),
        )
        .unwrap();

        let req = SandboxRequest::new("echo", serde_json::json!({}))
            .with_capability(SandboxCapability::Net);
        let err = sb.execute(req).await.unwrap_err();
        assert!(err.is_policy_denial());
    }

    #[test]
    fn null_sandbox_advertises_none_tier() {
        let sb = NullSandbox::new(
            "n",
            SandboxPolicy::denied_by_default(),
            Arc::new(EchoTool),
            default_context(),
        )
        .unwrap();
        assert_eq!(sb.tier(), SandboxTier::None);
    }
}
