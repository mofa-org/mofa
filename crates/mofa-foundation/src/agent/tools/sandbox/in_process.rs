//! In-process enforcing sandbox backend.
//!
//! `InProcessSandbox` runs the wrapped tool in the same tokio runtime but
//! enforces the policy at every boundary the tool exposes: declared
//! capabilities are prechecked, the wall-clock timeout is honoured via
//! `tokio::time::timeout`, and output size is capped post-execution.
//!
//! This sandbox is appropriate for **semi-trusted** tools that are known not
//! to escape the invocation boundary on their own — typically tools written
//! as a `Tool` trait impl in Rust, where the *potential* misuse is bounded
//! and the cost of fork/exec is not justified.
//!
//! For genuinely untrusted code, prefer [`super::ChildProcessSandbox`] or a
//! `WasmtimeSandbox` (follow-up PR).
//!
//! Declared tier: [`SandboxTier::None`] — the sandbox enforces policy but
//! cannot protect against a tool that wilfully bypasses policy-aware APIs.

use std::sync::Arc;
use std::time::Duration;
use std::time::Instant;

use async_trait::async_trait;
use mofa_kernel::agent::components::sandbox::{
    SandboxError, SandboxExecutionStats, SandboxPolicy, SandboxRequest, SandboxResponse,
    SandboxResult, SandboxTier, ToolSandbox,
};
use mofa_kernel::agent::components::tool::{Tool, ToolInput};
use mofa_kernel::agent::context::AgentContext;
use tokio::time::timeout;

/// Same-process sandbox with policy precheck, wall-clock timeout, and
/// output-size enforcement.
pub struct InProcessSandbox {
    name: String,
    policy: SandboxPolicy,
    tool: Arc<
        dyn Tool<serde_json::Value, serde_json::Value> + Send + Sync + 'static,
    >,
    context: Arc<AgentContext>,
}

impl InProcessSandbox {
    /// Wrap a tool in an in-process sandbox.
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

    fn effective_wall_timeout(&self) -> Duration {
        self.policy
            .resource_limits()
            .wall_timeout
            .unwrap_or(Duration::from_secs(300))
    }
}

#[async_trait]
impl ToolSandbox for InProcessSandbox {
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
        let wall = self.effective_wall_timeout();
        let tool_name_for_err = req.tool_name.clone();

        let exec_fut = self.tool.execute(tool_input, &self.context);
        let tool_result = match timeout(wall, exec_fut).await {
            Ok(result) => result,
            Err(_) => {
                return Err(SandboxError::WallTimeout {
                    tool: tool_name_for_err,
                    limit: wall,
                });
            }
        };

        let elapsed_ms = start.elapsed().as_millis() as u64;

        if !tool_result.success {
            return Err(SandboxError::BackendFailure(
                tool_result
                    .error
                    .unwrap_or_else(|| "tool reported failure with no error message".into()),
            ));
        }

        let output_str = tool_result.output.to_string();
        let output_bytes = output_str.len() as u64;

        if let Some(cap) = self.policy.resource_limits().output_limit_bytes {
            if output_bytes > cap {
                return Err(SandboxError::OutputTooLarge {
                    tool: tool_name_for_err,
                    limit_bytes: cap,
                    observed_bytes: output_bytes,
                });
            }
        }

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
    use mofa_kernel::agent::components::sandbox::{
        SandboxCapability, SandboxPolicy, SandboxResourceLimits,
    };
    use mofa_kernel::agent::components::tool::{ToolMetadata, ToolResult};

    struct EchoTool;

    #[async_trait]
    impl Tool<serde_json::Value, serde_json::Value> for EchoTool {
        fn name(&self) -> &str {
            "echo"
        }
        fn description(&self) -> &str {
            ""
        }
        fn parameters_schema(&self) -> serde_json::Value {
            serde_json::json!({})
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

    struct SlowTool {
        sleep: Duration,
    }

    #[async_trait]
    impl Tool<serde_json::Value, serde_json::Value> for SlowTool {
        fn name(&self) -> &str {
            "slow"
        }
        fn description(&self) -> &str {
            ""
        }
        fn parameters_schema(&self) -> serde_json::Value {
            serde_json::json!({})
        }
        async fn execute(
            &self,
            _input: ToolInput<serde_json::Value>,
            _ctx: &AgentContext,
        ) -> ToolResult<serde_json::Value> {
            tokio::time::sleep(self.sleep).await;
            ToolResult::success(serde_json::json!({"ok": true}))
        }
        fn metadata(&self) -> ToolMetadata {
            ToolMetadata::default()
        }
    }

    struct LargeOutputTool {
        payload_len: usize,
    }

    #[async_trait]
    impl Tool<serde_json::Value, serde_json::Value> for LargeOutputTool {
        fn name(&self) -> &str {
            "large"
        }
        fn description(&self) -> &str {
            ""
        }
        fn parameters_schema(&self) -> serde_json::Value {
            serde_json::json!({})
        }
        async fn execute(
            &self,
            _input: ToolInput<serde_json::Value>,
            _ctx: &AgentContext,
        ) -> ToolResult<serde_json::Value> {
            ToolResult::success(serde_json::Value::String("x".repeat(self.payload_len)))
        }
        fn metadata(&self) -> ToolMetadata {
            ToolMetadata::default()
        }
    }

    fn default_context() -> Arc<AgentContext> {
        Arc::new(AgentContext::new("test-agent"))
    }

    #[tokio::test]
    async fn in_process_sandbox_runs_successful_tool() {
        let sb = InProcessSandbox::new(
            "inp-echo",
            SandboxPolicy::builder()
                .allow(SandboxCapability::Compute)
                .build()
                .unwrap(),
            Arc::new(EchoTool),
            default_context(),
        )
        .unwrap();
        let req = SandboxRequest::new("echo", serde_json::json!({"x": 1}))
            .with_capability(SandboxCapability::Compute);
        let resp = sb.execute(req).await.unwrap();
        assert_eq!(resp.output, serde_json::json!({"x": 1}));
    }

    #[tokio::test]
    async fn in_process_sandbox_enforces_wall_timeout() {
        let policy = SandboxPolicy::builder()
            .allow(SandboxCapability::Compute)
            .resource_limits(SandboxResourceLimits {
                wall_timeout: Some(Duration::from_millis(50)),
                cpu_time_limit: None,
                memory_limit_bytes: None,
                output_limit_bytes: None,
                max_open_files: None,
            })
            .build()
            .unwrap();
        let sb = InProcessSandbox::new(
            "inp-slow",
            policy,
            Arc::new(SlowTool {
                sleep: Duration::from_secs(5),
            }),
            default_context(),
        )
        .unwrap();
        let req = SandboxRequest::new("slow", serde_json::json!({}))
            .with_capability(SandboxCapability::Compute);
        let err = sb.execute(req).await.unwrap_err();
        assert!(matches!(err, SandboxError::WallTimeout { .. }));
    }

    #[tokio::test]
    async fn in_process_sandbox_enforces_output_cap() {
        let policy = SandboxPolicy::builder()
            .allow(SandboxCapability::Compute)
            .resource_limits(SandboxResourceLimits {
                wall_timeout: Some(Duration::from_secs(5)),
                cpu_time_limit: None,
                memory_limit_bytes: None,
                output_limit_bytes: Some(32),
                max_open_files: None,
            })
            .build()
            .unwrap();
        let sb = InProcessSandbox::new(
            "inp-large",
            policy,
            Arc::new(LargeOutputTool { payload_len: 1024 }),
            default_context(),
        )
        .unwrap();
        let req = SandboxRequest::new("large", serde_json::json!({}))
            .with_capability(SandboxCapability::Compute);
        let err = sb.execute(req).await.unwrap_err();
        assert!(matches!(err, SandboxError::OutputTooLarge { .. }));
    }

    #[tokio::test]
    async fn in_process_sandbox_denies_undeclared_capability() {
        let sb = InProcessSandbox::new(
            "inp-deny",
            SandboxPolicy::denied_by_default(),
            Arc::new(EchoTool),
            default_context(),
        )
        .unwrap();
        let req = SandboxRequest::new("echo", serde_json::json!({}))
            .with_capability(SandboxCapability::Subprocess);
        let err = sb.execute(req).await.unwrap_err();
        assert!(err.is_policy_denial());
    }
}
