//! Tool execution sandbox demo.
//!
//! Runs three scenarios end-to-end:
//!
//! 1. Trusted tool under `NullSandbox` — passthrough, succeeds.
//! 2. Semi-trusted tool under `InProcessSandbox` with a tight wall-clock
//!    timeout — slow tool gets cancelled with `WallTimeout`.
//! 3. "Malicious" tool that tries to reach a disallowed network host —
//!    blocked by policy with `NetworkNotAllowed`.
//!
//! Each scenario prints the backend tier, the policy summary, and the
//! observed sandbox response or error.
//!
//! Run:
//!
//! ```bash
//! cargo run -p tool_sandbox_demo
//! ```

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use mofa_foundation::agent::tools::sandbox::{InProcessSandbox, NullSandbox};
use mofa_kernel::agent::components::sandbox::{
    NetEndpoint, SandboxCapability, SandboxError, SandboxPolicy, SandboxRequest,
    SandboxResourceLimits, ToolSandbox,
};
use mofa_kernel::agent::components::tool::{Tool, ToolInput, ToolMetadata, ToolResult};
use mofa_kernel::agent::context::AgentContext;

// ---------------------------------------------------------------------------
// Demo tools
// ---------------------------------------------------------------------------

struct TrustedCalculatorTool;

#[async_trait]
impl Tool<serde_json::Value, serde_json::Value> for TrustedCalculatorTool {
    fn name(&self) -> &str {
        "calc"
    }
    fn description(&self) -> &str {
        "Add two numbers"
    }
    fn parameters_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "a": {"type": "number"},
                "b": {"type": "number"}
            },
            "required": ["a", "b"]
        })
    }
    async fn execute(
        &self,
        input: ToolInput<serde_json::Value>,
        _ctx: &AgentContext,
    ) -> ToolResult<serde_json::Value> {
        let a = input.get_number("a").unwrap_or(0.0);
        let b = input.get_number("b").unwrap_or(0.0);
        ToolResult::success(serde_json::json!({"sum": a + b}))
    }
    fn metadata(&self) -> ToolMetadata {
        ToolMetadata::default()
    }
}

struct SlowTool;

#[async_trait]
impl Tool<serde_json::Value, serde_json::Value> for SlowTool {
    fn name(&self) -> &str {
        "slow"
    }
    fn description(&self) -> &str {
        "Simulates a runaway computation"
    }
    fn parameters_schema(&self) -> serde_json::Value {
        serde_json::json!({"type": "object"})
    }
    async fn execute(
        &self,
        _input: ToolInput<serde_json::Value>,
        _ctx: &AgentContext,
    ) -> ToolResult<serde_json::Value> {
        tokio::time::sleep(Duration::from_secs(60)).await;
        ToolResult::success(serde_json::json!({"done": true}))
    }
    fn metadata(&self) -> ToolMetadata {
        ToolMetadata::default()
    }
}

// ---------------------------------------------------------------------------
// Scenarios
// ---------------------------------------------------------------------------

async fn scenario_trusted_calculator() {
    println!("\n=== Scenario 1: Trusted tool under NullSandbox ===");
    let policy = SandboxPolicy::builder()
        .allow(SandboxCapability::Compute)
        .build()
        .expect("policy");
    let ctx = Arc::new(AgentContext::new("demo"));
    let sb = NullSandbox::new("calc", policy, Arc::new(TrustedCalculatorTool), ctx)
        .expect("sandbox");

    let req = SandboxRequest::new("calc", serde_json::json!({"a": 41, "b": 1}))
        .with_capability(SandboxCapability::Compute);

    println!("  tier   : {:?}", sb.tier());
    println!("  policy : Compute only");
    match sb.execute(req).await {
        Ok(resp) => println!(
            "  result : OK, output={}, wall_ms={:?}",
            resp.output, resp.stats.wall_time_ms
        ),
        Err(e) => println!("  result : ERR {e}"),
    }
}

async fn scenario_runaway_computation_timeout() {
    println!("\n=== Scenario 2: Slow tool under InProcessSandbox (wall timeout) ===");
    let policy = SandboxPolicy::builder()
        .allow(SandboxCapability::Compute)
        .resource_limits(SandboxResourceLimits {
            wall_timeout: Some(Duration::from_millis(200)),
            cpu_time_limit: None,
            memory_limit_bytes: None,
            output_limit_bytes: None,
            max_open_files: None,
        })
        .build()
        .expect("policy");
    let ctx = Arc::new(AgentContext::new("demo"));
    let sb = InProcessSandbox::new("slow", policy, Arc::new(SlowTool), ctx).expect("sandbox");

    let req = SandboxRequest::new("slow", serde_json::json!({}))
        .with_capability(SandboxCapability::Compute);

    println!("  tier   : {:?}", sb.tier());
    println!("  policy : Compute only, wall_timeout=200ms");
    match sb.execute(req).await {
        Ok(resp) => println!("  result : OK {}", resp.output),
        Err(e) => {
            let class = if e.is_resource_limit() {
                "resource-limit"
            } else if e.is_policy_denial() {
                "policy-denial"
            } else {
                "backend-failure"
            };
            println!("  result : ERR [{class}] {e}");
        }
    }
}

async fn scenario_malicious_network_access_blocked() {
    println!("\n=== Scenario 3: Policy blocks disallowed network destination ===");
    let policy = SandboxPolicy::builder()
        .allow(SandboxCapability::Net)
        .allow_net(NetEndpoint::HostPort {
            host: "api.trusted.example".into(),
            port: 443,
        })
        .build()
        .expect("policy");

    println!("  policy : Net → api.trusted.example:443");
    println!("  tool declares attempted destination via policy.check_net():");

    match policy.check_net("exfil-tool", "evil.attacker.example", 443) {
        Ok(()) => println!("  result : unexpectedly allowed"),
        Err(SandboxError::NetworkNotAllowed { host, port, .. }) => {
            println!("  result : DENIED  host={host} port={port}")
        }
        Err(other) => println!("  result : ERR {other}"),
    }
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    println!("tool_sandbox_demo — capability-scoped tool execution");
    scenario_trusted_calculator().await;
    scenario_runaway_computation_timeout().await;
    scenario_malicious_network_access_blocked().await;
    println!();
}
