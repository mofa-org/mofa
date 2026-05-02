//! Sandbox trait contracts
//!
//! Backend-agnostic interface for sandboxed tool execution.
//!
//! ```text
//!            ┌────────────────────────────────────────┐
//!            │                Tool                     │
//!            │  (from agent::components::tool::Tool)   │
//!            └──────────────────┬─────────────────────┘
//!                               │
//!                               │ wrapped by
//!                               ▼
//!            ┌────────────────────────────────────────┐
//!            │          SandboxedTool<T>               │
//!            │                                         │
//!            │  ┌─────────┐   ┌──────────────────┐    │
//!            │  │  tool   │──▶│   ToolSandbox    │    │
//!            │  └─────────┘   │   (trait obj)    │    │
//!            │                └─────────┬────────┘    │
//!            │                          │              │
//!            │                          ▼              │
//!            │          ┌───────────────────────┐     │
//!            │          │    SandboxPolicy      │     │
//!            │          └───────────────────────┘     │
//!            └────────────────────────────────────────┘
//!                               │
//!                               │ dispatches to one of
//!                               ▼
//!          ┌───────────┬───────────────┬────────────┐
//!          │           │               │            │
//!          ▼           ▼               ▼            ▼
//!      NullSandbox  ProcessSandbox  WasmtimeSbx   (custom)
//!       (trusted)   (foundation)    (foundation)  (user)
//! ```

use super::error::{SandboxError, SandboxResult};
use super::policy::SandboxPolicy;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

/// A tiered classification of the isolation strength offered by a backend.
///
/// This is advisory metadata — callers can surface it in audit logs or use
/// it to refuse a particular backend for high-risk tools.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[non_exhaustive]
pub enum SandboxTier {
    /// No isolation at all. Pass-through execution; policy is checked but
    /// never enforced on the tool's host-side syscalls.
    None,
    /// OS-level isolation via a separate process with resource limits.
    /// Defends against accidental misuse; not a hermetic boundary against
    /// sophisticated adversaries without additional kernel-level isolation.
    Process,
    /// Hermetic language-VM isolation (wasmtime).  Denies all syscalls by
    /// default and requires explicit host function imports for I/O.
    LanguageVm,
    /// Full OS-level virtualization (containers, microVMs). Reserved for
    /// downstream backends; the in-tree backends stop at `LanguageVm`.
    Virtualized,
}

impl SandboxTier {
    pub fn as_str(&self) -> &'static str {
        match self {
            SandboxTier::None => "None",
            SandboxTier::Process => "Process",
            SandboxTier::LanguageVm => "LanguageVm",
            SandboxTier::Virtualized => "Virtualized",
        }
    }
}

/// Metrics captured by a sandbox for a single tool invocation.
///
/// Backends populate the fields they can observe; unsupported fields stay
/// `None`. This is the observability surface that downstream audit logs
/// and telemetry exporters consume.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SandboxExecutionStats {
    /// Wall-clock time elapsed in the sandbox.
    pub wall_time_ms: Option<u64>,
    /// CPU time (user+sys) consumed inside the sandbox.
    pub cpu_time_ms: Option<u64>,
    /// Peak resident memory observed.
    pub peak_memory_bytes: Option<u64>,
    /// Bytes read from stdin / inputs.
    pub input_bytes: Option<u64>,
    /// Bytes of captured output.
    pub output_bytes: Option<u64>,
    /// Count of capability-denial events intercepted during execution.
    /// Useful for flagging misbehaving tools that consistently probe the
    /// policy boundary.
    pub denials: u32,
}

/// A structured, capability-annotated request to run a tool inside a sandbox.
///
/// This is the *input* half of the sandbox contract. The tool's business
/// arguments travel as opaque JSON; the sandbox only cares about
/// identifying the tool and knowing which capabilities the invocation will
/// exercise (hint — backends may use this for up-front denial before
/// entering the isolate at all).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SandboxRequest {
    /// The tool's unique name (matches `Tool::name()`).
    pub tool_name: String,
    /// Serialized JSON arguments passed through to the tool unchanged.
    pub arguments: serde_json::Value,
    /// Capabilities the invocation is declared to require.
    ///
    /// This is a hint to the sandbox for fast-path denial. If a tool
    /// attempts an operation it did not declare here and the policy does
    /// not grant that capability, the sandbox still denies at runtime.
    pub declared_capabilities: Vec<super::policy::SandboxCapability>,
}

impl SandboxRequest {
    pub fn new(tool_name: impl Into<String>, arguments: serde_json::Value) -> Self {
        Self {
            tool_name: tool_name.into(),
            arguments,
            declared_capabilities: Vec::new(),
        }
    }

    pub fn with_capability(mut self, cap: super::policy::SandboxCapability) -> Self {
        self.declared_capabilities.push(cap);
        self
    }
}

/// Successful output of a sandboxed tool call.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SandboxResponse {
    /// Tool output (opaque JSON, shape defined by the tool).
    pub output: serde_json::Value,
    /// Execution statistics from the sandbox.
    pub stats: SandboxExecutionStats,
}

/// The core backend-agnostic trait implemented by every sandbox backend.
///
/// Implementors are typically concrete types in `mofa-foundation` such as
/// `NullSandbox`, `ProcessSandbox`, and `WasmtimeSandbox`. The kernel only
/// defines the contract here.
#[async_trait]
pub trait ToolSandbox: Send + Sync {
    /// A short name for the backend (used in telemetry and logs).
    fn name(&self) -> &str;

    /// Declared isolation tier.  Implementations must not overstate this —
    /// a pass-through `NullSandbox` must return [`SandboxTier::None`].
    fn tier(&self) -> SandboxTier;

    /// The policy this backend will enforce.
    fn policy(&self) -> &SandboxPolicy;

    /// Run a single tool invocation through the sandbox.
    ///
    /// The sandbox is responsible for:
    /// 1. Applying fast-path capability denial based on
    ///    `req.declared_capabilities`.
    /// 2. Entering the isolate (fork, wasmtime instantiation, ...).
    /// 3. Passing `req.arguments` to the tool and capturing the output.
    /// 4. Enforcing resource limits during execution.
    /// 5. Returning either `SandboxResponse` with stats, or a
    ///    categorised `SandboxError`.
    async fn execute(&self, req: SandboxRequest) -> SandboxResult<SandboxResponse>;

    /// Best-effort precheck — returns `Ok(())` if the declared capabilities
    /// on `req` are all permitted by this backend's policy, otherwise the
    /// specific denial. This does not run the tool.
    ///
    /// Default implementation walks `req.declared_capabilities` against
    /// [`SandboxPolicy::grants`].
    fn precheck(&self, req: &SandboxRequest) -> SandboxResult<()> {
        for cap in &req.declared_capabilities {
            if !self.policy().grants(*cap) {
                return Err(SandboxError::CapabilityDenied {
                    tool: req.tool_name.clone(),
                    capability: cap.as_str().into(),
                    allowed: self
                        .policy()
                        .allowed_capabilities()
                        .iter()
                        .map(|c| c.as_str().to_string())
                        .collect(),
                });
            }
        }
        Ok(())
    }
}

/// Decision emitted by a [`SandboxObserver`] for a single request.
///
/// Observers are read-mostly: they receive notifications and can emit an
/// advisory `Decision` for audit trails, but the sandbox backend does the
/// actual allow/deny.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ObservationDecision {
    Allowed,
    Denied,
    ResourceBreach,
    BackendError,
}

/// Hook invoked before and after every sandboxed execution.
///
/// Intended for audit logging, metrics export, and policy-drift detection.
/// Implementors must be fast — observers run on the critical path.
#[async_trait]
pub trait SandboxObserver: Send + Sync {
    /// Called before the sandbox attempts to run `req`.
    async fn before(&self, backend: &str, tier: SandboxTier, req: &SandboxRequest);

    /// Called after execution (success or failure).
    async fn after(
        &self,
        backend: &str,
        tier: SandboxTier,
        req: &SandboxRequest,
        decision: ObservationDecision,
        stats: &SandboxExecutionStats,
    );
}

#[cfg(test)]
mod tests {
    use super::super::policy::{SandboxCapability, SandboxPolicy};
    use super::*;
    use std::sync::Mutex;
    use std::sync::atomic::{AtomicU32, Ordering};

    /// Minimal in-kernel sandbox implementation used solely to exercise
    /// the trait contract end-to-end in tests. Real backends live in
    /// `mofa-foundation` per the microkernel layering rule.
    struct PassthroughSandbox {
        policy: SandboxPolicy,
    }

    #[async_trait]
    impl ToolSandbox for PassthroughSandbox {
        fn name(&self) -> &str {
            "passthrough-test"
        }
        fn tier(&self) -> SandboxTier {
            SandboxTier::None
        }
        fn policy(&self) -> &SandboxPolicy {
            &self.policy
        }
        async fn execute(&self, req: SandboxRequest) -> SandboxResult<SandboxResponse> {
            self.precheck(&req)?;
            // Echo arguments as output; populate realistic stats so
            // downstream observability hooks have something to record.
            let size = req.arguments.to_string().len() as u64;
            Ok(SandboxResponse {
                output: req.arguments.clone(),
                stats: SandboxExecutionStats {
                    wall_time_ms: Some(1),
                    cpu_time_ms: Some(1),
                    input_bytes: Some(size),
                    output_bytes: Some(size),
                    ..Default::default()
                },
            })
        }
    }

    #[derive(Default)]
    struct CountingObserver {
        before: AtomicU32,
        after: AtomicU32,
        last_decision: Mutex<Option<ObservationDecision>>,
    }

    #[async_trait]
    impl SandboxObserver for CountingObserver {
        async fn before(&self, _: &str, _: SandboxTier, _: &SandboxRequest) {
            self.before.fetch_add(1, Ordering::SeqCst);
        }
        async fn after(
            &self,
            _: &str,
            _: SandboxTier,
            _: &SandboxRequest,
            decision: ObservationDecision,
            _: &SandboxExecutionStats,
        ) {
            self.after.fetch_add(1, Ordering::SeqCst);
            *self.last_decision.lock().unwrap() = Some(decision);
        }
    }

    // ---------- request / response shaping ----------

    #[test]
    fn sandbox_request_builder_adds_capabilities() {
        let req = SandboxRequest::new("calc", serde_json::json!({"x": 1}))
            .with_capability(SandboxCapability::Compute)
            .with_capability(SandboxCapability::Clock);
        assert_eq!(req.declared_capabilities.len(), 2);
        assert_eq!(req.tool_name, "calc");
    }

    #[test]
    fn sandbox_request_is_json_roundtrippable() {
        let req = SandboxRequest::new("t", serde_json::json!({"a": 1}))
            .with_capability(SandboxCapability::Compute);
        let s = serde_json::to_string(&req).unwrap();
        let parsed: SandboxRequest = serde_json::from_str(&s).unwrap();
        assert_eq!(parsed.tool_name, req.tool_name);
        assert_eq!(parsed.declared_capabilities, req.declared_capabilities);
    }

    // ---------- precheck ----------

    #[test]
    fn precheck_rejects_undeclared_capability() {
        let sb = PassthroughSandbox {
            policy: SandboxPolicy::denied_by_default(),
        };
        let req = SandboxRequest::new("bad", serde_json::json!({}))
            .with_capability(SandboxCapability::Net);
        let err = sb.precheck(&req).unwrap_err();
        assert!(matches!(err, SandboxError::CapabilityDenied { .. }));
    }

    #[test]
    fn precheck_allows_implicit_compute() {
        let sb = PassthroughSandbox {
            policy: SandboxPolicy::denied_by_default(),
        };
        let req = SandboxRequest::new("good", serde_json::json!({}))
            .with_capability(SandboxCapability::Compute);
        assert!(sb.precheck(&req).is_ok());
    }

    #[test]
    fn precheck_allows_explicit_granted() {
        let policy = SandboxPolicy::builder()
            .allow(SandboxCapability::EnvRead)
            .allow_env("HOME")
            .build()
            .unwrap();
        let sb = PassthroughSandbox { policy };
        let req = SandboxRequest::new("env", serde_json::json!({}))
            .with_capability(SandboxCapability::EnvRead);
        assert!(sb.precheck(&req).is_ok());
    }

    #[test]
    fn precheck_reports_allowed_set_in_denial() {
        let policy = SandboxPolicy::builder()
            .allow(SandboxCapability::Clock)
            .build()
            .unwrap();
        let sb = PassthroughSandbox { policy };
        let req = SandboxRequest::new("bad", serde_json::json!({}))
            .with_capability(SandboxCapability::Net);
        let err = sb.precheck(&req).unwrap_err();
        match err {
            SandboxError::CapabilityDenied { allowed, .. } => {
                assert!(allowed.contains(&"Clock".to_string()));
            }
            other => panic!("unexpected error: {other:?}"),
        }
    }

    // ---------- end-to-end execute round-trip ----------

    #[tokio::test]
    async fn execute_round_trip_returns_response_with_stats() {
        let policy = SandboxPolicy::builder()
            .allow(SandboxCapability::Clock)
            .build()
            .unwrap();
        let sb = PassthroughSandbox { policy };
        let req = SandboxRequest::new("echo", serde_json::json!({"msg": "hello"}))
            .with_capability(SandboxCapability::Clock);
        let resp = sb.execute(req.clone()).await.expect("must succeed");
        assert_eq!(resp.output, req.arguments);
        assert_eq!(resp.stats.wall_time_ms, Some(1));
        assert!(resp.stats.input_bytes.is_some());
    }

    #[tokio::test]
    async fn execute_denies_undeclared_capability_through_precheck() {
        let sb = PassthroughSandbox {
            policy: SandboxPolicy::denied_by_default(),
        };
        let req = SandboxRequest::new("evil", serde_json::json!({}))
            .with_capability(SandboxCapability::Subprocess);
        let err = sb.execute(req).await.unwrap_err();
        assert!(err.is_policy_denial());
    }

    // ---------- observer hook wiring ----------

    #[tokio::test]
    async fn observer_fires_before_and_after_success() {
        let observer = CountingObserver::default();
        let policy = SandboxPolicy::builder()
            .allow(SandboxCapability::Clock)
            .build()
            .unwrap();
        let sb = PassthroughSandbox { policy };
        let req = SandboxRequest::new("t", serde_json::json!({}))
            .with_capability(SandboxCapability::Clock);

        observer.before(sb.name(), sb.tier(), &req).await;
        let resp = sb.execute(req.clone()).await.unwrap();
        observer
            .after(
                sb.name(),
                sb.tier(),
                &req,
                ObservationDecision::Allowed,
                &resp.stats,
            )
            .await;

        assert_eq!(observer.before.load(Ordering::SeqCst), 1);
        assert_eq!(observer.after.load(Ordering::SeqCst), 1);
        assert_eq!(
            *observer.last_decision.lock().unwrap(),
            Some(ObservationDecision::Allowed)
        );
    }

    #[tokio::test]
    async fn observer_records_denial() {
        let observer = CountingObserver::default();
        let sb = PassthroughSandbox {
            policy: SandboxPolicy::denied_by_default(),
        };
        let req = SandboxRequest::new("t", serde_json::json!({}))
            .with_capability(SandboxCapability::Net);

        observer.before(sb.name(), sb.tier(), &req).await;
        let err = sb.execute(req.clone()).await.unwrap_err();
        observer
            .after(
                sb.name(),
                sb.tier(),
                &req,
                ObservationDecision::Denied,
                &SandboxExecutionStats::default(),
            )
            .await;

        assert!(err.is_policy_denial());
        assert_eq!(
            *observer.last_decision.lock().unwrap(),
            Some(ObservationDecision::Denied)
        );
    }

    // ---------- tier semantics ----------

    #[test]
    fn tier_ordering_reflects_isolation_strength() {
        assert!(SandboxTier::None < SandboxTier::Process);
        assert!(SandboxTier::Process < SandboxTier::LanguageVm);
        assert!(SandboxTier::LanguageVm < SandboxTier::Virtualized);
    }

    #[test]
    fn tier_as_str_is_non_empty_for_every_variant() {
        for t in [
            SandboxTier::None,
            SandboxTier::Process,
            SandboxTier::LanguageVm,
            SandboxTier::Virtualized,
        ] {
            assert!(!t.as_str().is_empty());
        }
    }

    // ---------- combinatorial capability coverage ----------

    #[test]
    fn every_capability_has_consistent_display_and_as_str() {
        for cap in SandboxCapability::iter_all() {
            let s1 = cap.as_str();
            let s2 = format!("{cap}");
            assert_eq!(s1, s2);
        }
    }

    #[test]
    fn capability_iter_all_is_comprehensive() {
        // If a new capability is added, iter_all must be updated and this
        // test re-counted.  Currently 8 concrete variants.
        assert_eq!(SandboxCapability::iter_all().count(), 8);
    }
}
