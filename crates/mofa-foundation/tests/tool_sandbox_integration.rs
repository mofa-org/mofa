//! End-to-end integration tests for tool-execution sandboxing.
//!
//! Exercises each foundation backend against the kernel `ToolSandbox`
//! contract and verifies:
//!
//! - Happy path: declared capability permitted, tool runs, response
//!   carries stats.
//! - Capability-denial path: the sandbox refuses a tool call that declares
//!   a capability the policy does not grant.
//! - Resource-breach path: the sandbox enforces the wall-clock timeout and
//!   the output-size cap.
//! - Adapter wiring: the `SandboxedTool` wrapper makes any sandbox look
//!   like a regular `Tool` that a registry can hold.

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use mofa_foundation::agent::tools::sandbox::{
    ChildProcessCommand, ChildProcessSandbox, InProcessSandbox, NullSandbox, SandboxedTool,
};
use mofa_kernel::agent::components::sandbox::{
    SandboxCapability, SandboxError, SandboxPolicy, SandboxRequest, SandboxResourceLimits,
    SandboxTier, ToolSandbox,
};
use mofa_kernel::agent::components::tool::{Tool, ToolInput, ToolMetadata, ToolResult};
use mofa_kernel::agent::context::AgentContext;

// ---------------------------------------------------------------------------
// Fixture tools
// ---------------------------------------------------------------------------

struct EchoTool;

#[async_trait]
impl Tool<serde_json::Value, serde_json::Value> for EchoTool {
    fn name(&self) -> &str {
        "echo"
    }
    fn description(&self) -> &str {
        "Echo input as output"
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

fn ctx() -> Arc<AgentContext> {
    Arc::new(AgentContext::new("integration-test"))
}

// ---------------------------------------------------------------------------
// NullSandbox end-to-end
// ---------------------------------------------------------------------------

#[tokio::test]
async fn null_sandbox_end_to_end_happy_path() {
    let policy = SandboxPolicy::builder()
        .allow(SandboxCapability::Compute)
        .build()
        .unwrap();
    let sb = NullSandbox::new("null", policy, Arc::new(EchoTool), ctx()).unwrap();
    let req = SandboxRequest::new("echo", serde_json::json!({"hello": "world"}))
        .with_capability(SandboxCapability::Compute);
    let resp = sb.execute(req).await.unwrap();
    assert_eq!(resp.output, serde_json::json!({"hello": "world"}));
    assert!(resp.stats.wall_time_ms.is_some());
    assert_eq!(sb.tier(), SandboxTier::None);
}

#[tokio::test]
async fn null_sandbox_denies_undeclared_capability() {
    let sb = NullSandbox::new(
        "null",
        SandboxPolicy::denied_by_default(),
        Arc::new(EchoTool),
        ctx(),
    )
    .unwrap();
    let req = SandboxRequest::new("echo", serde_json::json!({}))
        .with_capability(SandboxCapability::Subprocess);
    let err = sb.execute(req).await.unwrap_err();
    assert!(err.is_policy_denial());
}

// ---------------------------------------------------------------------------
// InProcessSandbox end-to-end
// ---------------------------------------------------------------------------

#[tokio::test]
async fn in_process_sandbox_runs_tool() {
    let policy = SandboxPolicy::builder()
        .allow(SandboxCapability::Compute)
        .build()
        .unwrap();
    let sb = InProcessSandbox::new("inp", policy, Arc::new(EchoTool), ctx()).unwrap();
    let req = SandboxRequest::new("echo", serde_json::json!({"x": 42}))
        .with_capability(SandboxCapability::Compute);
    let resp = sb.execute(req).await.unwrap();
    assert_eq!(resp.output, serde_json::json!({"x": 42}));
}

#[tokio::test]
async fn in_process_sandbox_wall_timeout_fires() {
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
        "slow",
        policy,
        Arc::new(SlowTool {
            sleep: Duration::from_secs(5),
        }),
        ctx(),
    )
    .unwrap();
    let req = SandboxRequest::new("slow", serde_json::json!({}))
        .with_capability(SandboxCapability::Compute);
    let err = sb.execute(req).await.unwrap_err();
    assert!(matches!(err, SandboxError::WallTimeout { .. }));
    assert!(err.is_resource_limit());
}

// ---------------------------------------------------------------------------
// ChildProcessSandbox end-to-end
// ---------------------------------------------------------------------------

/// `cat` is present on every Unix and reads stdin → writes stdout unchanged,
/// which makes it a perfect fixture for JSON round-trip testing.
#[cfg(unix)]
#[tokio::test]
async fn child_process_sandbox_roundtrips_json_through_cat() {
    let policy = SandboxPolicy::builder()
        .allow(SandboxCapability::Subprocess)
        .allow_subprocess("cat")
        .build()
        .unwrap();
    let sb = ChildProcessSandbox::new(
        "cat-echo",
        policy,
        ChildProcessCommand::new("cat", vec![]),
    )
    .unwrap();
    let req = SandboxRequest::new("cat", serde_json::json!({"round": "trip"}))
        .with_capability(SandboxCapability::Subprocess);
    let resp = sb.execute(req).await.unwrap();
    assert_eq!(resp.output, serde_json::json!({"round": "trip"}));
    assert_eq!(sb.tier(), SandboxTier::Process);
}

#[cfg(unix)]
#[tokio::test]
async fn child_process_sandbox_enforces_wall_timeout() {
    let policy = SandboxPolicy::builder()
        .allow(SandboxCapability::Subprocess)
        .allow_subprocess("sleep")
        .resource_limits(SandboxResourceLimits {
            wall_timeout: Some(Duration::from_millis(100)),
            cpu_time_limit: None,
            memory_limit_bytes: None,
            output_limit_bytes: None,
            max_open_files: None,
        })
        .build()
        .unwrap();
    let sb = ChildProcessSandbox::new(
        "slow",
        policy,
        ChildProcessCommand::new("sleep", vec!["5".into()]),
    )
    .unwrap();
    let req = SandboxRequest::new("sleep", serde_json::json!({}))
        .with_capability(SandboxCapability::Subprocess);
    let err = sb.execute(req).await.unwrap_err();
    assert!(matches!(err, SandboxError::WallTimeout { .. }));
}

#[tokio::test]
async fn child_process_sandbox_rejects_disallowed_program_at_construction() {
    let policy = SandboxPolicy::builder()
        .allow(SandboxCapability::Subprocess)
        .allow_subprocess("cat")
        .build()
        .unwrap();
    let err = ChildProcessSandbox::new(
        "rogue",
        policy,
        ChildProcessCommand::new("bash", vec!["-c".into(), "rm -rf /".into()]),
    )
    .unwrap_err();
    assert!(matches!(err, SandboxError::SubprocessNotAllowed { .. }));
}

// ---------------------------------------------------------------------------
// SandboxedTool adapter
// ---------------------------------------------------------------------------

#[tokio::test]
async fn sandboxed_tool_adapter_routes_calls_through_sandbox() {
    let sb = Arc::new(
        NullSandbox::new(
            "null",
            SandboxPolicy::builder()
                .allow(SandboxCapability::Compute)
                .build()
                .unwrap(),
            Arc::new(EchoTool),
            ctx(),
        )
        .unwrap(),
    );
    let wrapped = SandboxedTool::new(
        "echo",
        "Sandboxed echo",
        serde_json::json!({"type": "object"}),
        sb,
    );
    assert_eq!(Tool::name(&wrapped), "echo");

    let input = ToolInput::from_json(serde_json::json!({"msg": "sandboxed"}));
    let result = wrapped.execute(input, &ctx()).await;
    assert!(result.success);
    assert_eq!(result.output, serde_json::json!({"msg": "sandboxed"}));
    assert!(result.metadata.contains_key("sandbox_wall_ms"));
    assert!(result.metadata.contains_key("sandbox_output_bytes"));
}
