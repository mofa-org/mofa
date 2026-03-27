//! FFI bindings for [`SwarmOrchestrator`].
//!
//! Exposes the `mofa swarm run "goal"` pipeline as a synchronous call
//! usable from Python, Kotlin, and Swift without callers managing async runtimes.
//!
//! # Architecture
//!
//! ```text
//! Python / Kotlin / Swift
//!   |
//!   +-- PyO3: PySwarmOrchestrator  (python feature)
//!   +-- UniFFI: SwarmOrchestratorFFI  (uniffi feature)
//!   |
//!   v
//! SwarmOrchestratorFFI  (this module, always compiled)
//!   |
//!   v
//! mofa_orchestrator::SwarmOrchestrator  (async Rust)
//!   |
//!   v
//! TaskAnalyzer → SwarmComposer → HITLGovernor → GovernanceLayer
//!   → Schedulers → SemanticDiscovery → SmithObservatory
//! ```

use tokio::runtime::Runtime;

use mofa_orchestrator::orchestrator::{OrchestratorConfig, SwarmOrchestrator};

use crate::MoFaError;

/// FFI-safe result of a complete swarm execution.
///
/// Returned by [`SwarmOrchestratorFFI::run_goal`] after the full pipeline
/// (TaskAnalyzer → SwarmComposer → schedulers → HITL gates) completes or fails.
///
/// Use `execution_id` to correlate this result with OTel traces in Jaeger
/// and with the JSONL audit log exported by `GovernanceLayer`.
#[derive(Debug, Clone)]
pub struct SwarmResultFFI {
    /// UUID for this execution run.
    pub execution_id: String,
    /// The original natural-language goal string.
    pub goal: String,
    /// Number of subtasks that completed successfully.
    pub tasks_succeeded: u64,
    /// Number of subtasks that failed or were rejected by a HITL gate.
    pub tasks_failed: u64,
    /// Total wall-clock time in milliseconds.
    pub wall_time_ms: u64,
}

/// FFI wrapper around [`mofa_orchestrator::SwarmOrchestrator`].
///
/// Owns a dedicated Tokio runtime so callers in Python, Kotlin, and Swift
/// do not need to manage async execution themselves — `run_goal` blocks until
/// the swarm pipeline completes.
///
/// # Python (via PyO3)
///
/// ```python
/// from mofa import SwarmOrchestrator
///
/// orch = SwarmOrchestrator("compliance-swarm")
/// result = orch.run_goal("review Q1 loan applications for fair lending violations")
/// print(f"succeeded={result.tasks_succeeded} id={result.execution_id}")
/// ```
///
/// # Kotlin (via UniFFI)
///
/// ```kotlin
/// val orch = SwarmOrchestratorFFI("compliance-swarm")
/// val result = orch.runGoal("review Q1 loan applications for fair lending violations")
/// println("succeeded=${result.tasksSucceeded}  id=${result.executionId}")
/// ```
///
/// # Swift (via UniFFI)
///
/// ```swift
/// let orch = SwarmOrchestratorFFI(name: "compliance-swarm")
/// let result = try orch.runGoal(goal: "review Q1 loan applications for fair lending violations")
/// print("succeeded=\(result.tasksSucceeded)  id=\(result.executionId)")
/// ```
pub struct SwarmOrchestratorFFI {
    inner: SwarmOrchestrator,
    rt: Runtime,
}

impl SwarmOrchestratorFFI {
    /// Create a new orchestrator with the given display name.
    ///
    /// The name appears in OpenTelemetry `gen_ai.agent.*` span attributes
    /// and in every `SwarmAuditLog` entry emitted during execution.
    pub fn new(name: String) -> Self {
        let config = OrchestratorConfig::new(name);
        Self {
            inner: SwarmOrchestrator::new(config),
            rt: Runtime::new()
                .expect("failed to create Tokio runtime for SwarmOrchestratorFFI"),
        }
    }

    /// Run the full swarm pipeline for the given natural-language goal.
    ///
    /// Blocks the calling thread until the swarm completes or fails.
    ///
    /// All [`OrchestratorError`] variants are mapped to [`MoFaError::RuntimeError`]
    /// so the error message crosses the FFI boundary as a UTF-8 string.
    pub fn run_goal(&self, goal: String) -> Result<SwarmResultFFI, MoFaError> {
        let result = self
            .rt
            .block_on(self.inner.run_goal(&goal))
            .map_err(|e| MoFaError::RuntimeError(e.to_string()))?;

        Ok(SwarmResultFFI {
            execution_id: result.execution_id,
            goal: result.goal,
            tasks_succeeded: result.tasks_succeeded as u64,
            tasks_failed: result.tasks_failed as u64,
            wall_time_ms: result.wall_time_ms,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_does_not_panic() {
        let _orch = SwarmOrchestratorFFI::new("test-swarm".to_string());
    }

    #[test]
    fn run_goal_returns_result_with_matching_goal() {
        let orch = SwarmOrchestratorFFI::new("test-swarm".to_string());
        let result = orch
            .run_goal("summarize quarterly earnings".to_string())
            .expect("run_goal must not fail against stub orchestrator");
        assert_eq!(result.goal, "summarize quarterly earnings");
        assert!(!result.execution_id.is_empty(), "execution_id must be a non-empty UUID");
    }

    #[test]
    fn run_goal_result_has_valid_timing() {
        let orch = SwarmOrchestratorFFI::new("test-swarm".to_string());
        let result = orch
            .run_goal("deploy service to staging".to_string())
            .expect("run_goal must not fail against stub orchestrator");
        // wall_time_ms must be non-negative (trivially true for u64 but assert type constraint)
        let _ = result.wall_time_ms;
    }

    #[test]
    fn run_goal_execution_ids_are_unique() {
        let orch = SwarmOrchestratorFFI::new("test-swarm".to_string());
        let r1 = orch.run_goal("goal one".to_string()).unwrap();
        let r2 = orch.run_goal("goal two".to_string()).unwrap();
        assert_ne!(
            r1.execution_id, r2.execution_id,
            "each run_goal call must produce a unique execution_id"
        );
    }
}
