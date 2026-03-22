//! SwarmHITLGate: human-in-the-loop approval gate for swarm subtask execution.
//!
//! [`SwarmHITLGate`] wraps any [`SubtaskExecutorFn`] and intercepts subtasks
//! that require human review before they are allowed to run.  It delegates to
//! the existing production-grade [`ReviewManager`] infrastructure (PR #826),
//! which provides audit trails, rate limiting, and webhook notifications —
//! rather than reimplementing a custom approval channel.
//!
//! # Design
//!
//! ```text
//! ParallelScheduler / SequentialScheduler
//!         │
//!         ▼  (subtask ready to execute)
//!  SwarmHITLGate::wrap_executor()
//!         │
//!   should_intercept? ──No──► inner executor
//!         │Yes
//!         ▼
//!   ReviewManager::request_review()   ← stored, notified
//!         │
//!   ReviewManager::wait_for_review()  ← polls until resolved
//!         │
//!   Approved? ──Yes──► inner executor
//!         │No
//!         ▼
//!   Err(GlobalError::Other(...))      ← scheduler marks task Failed
//! ```
//!
//! The returned [`SubtaskExecutorFn`] is a drop-in replacement that can be
//! passed directly to either scheduler.

use std::sync::Arc;
use std::time::Duration;

use mofa_kernel::agent::types::error::{GlobalError, GlobalResult};
use mofa_kernel::hitl::{
    ExecutionStep, ExecutionTrace, ReviewContext, ReviewRequest, ReviewResponse, ReviewType,
};
use tracing::{info_span, Instrument};

use crate::hitl::manager::ReviewManager;
use crate::swarm::config::HITLMode;
use crate::swarm::dag::{RiskLevel, SwarmSubtask};
use crate::swarm::scheduler::SubtaskExecutorFn;

/// Gate that intercepts HITL-flagged subtasks before execution.
///
/// Construct one via [`SwarmHITLGate::new`], wrap your executor with
/// [`SwarmHITLGate::wrap_executor`], and pass the result to a scheduler.
///
/// ```rust,ignore
/// let gate = Arc::new(SwarmHITLGate::new(manager, HITLMode::Optional, dag.id.clone()));
/// let gated = gate.wrap_executor(inner_executor);
/// ParallelScheduler::new().execute(&mut dag, gated).await?;
/// ```
pub struct SwarmHITLGate {
    manager: Arc<ReviewManager>,
    mode: HITLMode,
    execution_id: String,
    /// Minimum risk level that triggers a review in `HITLMode::Optional`.
    /// Defaults to [`RiskLevel::High`].
    risk_threshold: RiskLevel,
    /// Maximum time to wait for a reviewer decision.
    /// `None` uses the `ReviewManager`'s configured default expiration (1 h).
    review_timeout: Option<Duration>,
}

impl SwarmHITLGate {
    /// Create a new gate backed by the given [`ReviewManager`].
    ///
    /// - `mode`: governs which tasks are intercepted (see [`HITLMode`]).
    /// - `execution_id`: identifier for the enclosing swarm execution,
    ///   forwarded to each [`ReviewRequest`] for audit correlation.
    pub fn new(
        manager: Arc<ReviewManager>,
        mode: HITLMode,
        execution_id: impl Into<String>,
    ) -> Self {
        Self {
            manager,
            mode,
            execution_id: execution_id.into(),
            risk_threshold: RiskLevel::High,
            review_timeout: None,
        }
    }

    /// Override the minimum risk level that triggers a review.
    ///
    /// Only relevant when `mode` is [`HITLMode::Optional`].
    pub fn with_risk_threshold(mut self, threshold: RiskLevel) -> Self {
        self.risk_threshold = threshold;
        self
    }

    /// Override the maximum wait time for reviewer decisions.
    pub fn with_review_timeout(mut self, timeout: Duration) -> Self {
        self.review_timeout = Some(timeout);
        self
    }

    /// Wrap an executor with HITL interception logic.
    ///
    /// Returns a new [`SubtaskExecutorFn`] that can be passed directly to
    /// [`SequentialScheduler::execute`] or [`ParallelScheduler::execute`].
    /// The gate is arc-cloned into each task closure so the original
    /// `Arc<SwarmHITLGate>` can be dropped by the caller after wrapping.
    pub fn wrap_executor(self: Arc<Self>, inner: SubtaskExecutorFn) -> SubtaskExecutorFn {
        Arc::new(move |idx, mut task: SwarmSubtask| {
            let gate = Arc::clone(&self);
            let inner = Arc::clone(&inner);
            let task_for_gate = task.clone();
            Box::pin(async move {
                let span = info_span!(
                    "swarm.hitl_gate",
                    task_id = %task_for_gate.id,
                    risk_level = ?task_for_gate.risk_level,
                    execution_id = %gate.execution_id,
                );
                async move {
                    if gate.should_intercept(&task_for_gate) {
                        let gate_span = info_span!(
                            "hitl.approval_gate",
                            review_id  = tracing::field::Empty,
                            decision   = tracing::field::Empty,
                            timed_out  = tracing::field::Empty,
                        );
                        let result = gate
                            .request_and_wait(&task_for_gate, &gate_span)
                            .instrument(gate_span.clone())
                            .await;

                        match result {
                            Ok(Some(modified_desc)) => {
                                task.description = modified_desc;
                            }
                            Ok(None) => {}
                            Err(e) if gate.is_optional() => {
                                tracing::warn!(
                                    task_id = %task_for_gate.id,
                                    "hitl timeout in Optional mode — auto-approving"
                                );
                                let _ = e;
                            }
                            Err(e) => return Err(e),
                        }
                    }
                    inner(idx, task)
                        .instrument(info_span!("swarm.subtask.execute"))
                        .await
                }
                .instrument(span)
                .await
            })
        })
    }

    fn is_optional(&self) -> bool {
        matches!(self.mode, HITLMode::Optional)
    }

    // ── Private ───────────────────────────────────────────────────────────

    /// Determine whether this task must be reviewed before execution.
    fn should_intercept(&self, task: &SwarmSubtask) -> bool {
        match self.mode {
            HITLMode::None => false,
            HITLMode::Required => true,
            HITLMode::Optional => {
                task.hitl_required || task.risk_level >= self.risk_threshold
            }
            // Guard against future HITLMode variants (#[non_exhaustive]).
            _ => false,
        }
    }

    /// Submit a [`ReviewRequest`] for the given task and block until a
    /// reviewer responds or the timeout expires.
    ///
    /// Returns `Ok(None)` when approved as-is, `Ok(Some(desc))` when the
    /// reviewer requested changes and provided a modified description, or
    /// `Err` when the task was rejected or an unexpected response was received.
    async fn request_and_wait(
        &self,
        task: &SwarmSubtask,
        span: &tracing::Span,
    ) -> GlobalResult<Option<String>> {
        let now_ms = chrono::Utc::now()
            .timestamp_millis()
            .try_into()
            .unwrap_or(u64::MAX);

        let trace = ExecutionTrace {
            steps: vec![ExecutionStep {
                step_id: task.id.clone(),
                step_type: "swarm_subtask".to_string(),
                timestamp_ms: now_ms,
                input: Some(serde_json::json!({
                    "description":            task.description,
                    "risk_level":             format!("{:?}", task.risk_level),
                    "complexity":             task.complexity,
                    "required_capabilities":  task.required_capabilities,
                    "estimated_duration_secs": task.estimated_duration_secs,
                })),
                output: None,
                metadata: Default::default(),
            }],
            duration_ms: task.estimated_duration_secs.unwrap_or(0).saturating_mul(1_000),
        };

        let context = ReviewContext::new(
            trace,
            serde_json::json!({
                "task_id":                task.id,
                "description":            task.description,
                "risk_level":             format!("{:?}", task.risk_level),
                "estimated_duration_secs": task.estimated_duration_secs,
                "required_capabilities":  task.required_capabilities,
            }),
        );

        let mut request =
            ReviewRequest::new(&self.execution_id, ReviewType::Approval, context)
                .with_node_id(&task.id);

        request.metadata.priority = task.risk_level.to_priority();
        request.metadata.tags = vec![
            "swarm-subtask".to_string(),
            format!("risk:{}", format!("{:?}", task.risk_level).to_lowercase()),
        ];

        let review_id = self
            .manager
            .request_review(request)
            .await
            .map_err(|e| GlobalError::Other(format!("HITL request_review failed: {e}")))?;

        span.record("review_id", review_id.as_str());

        let wait_result = self
            .manager
            .wait_for_review(&review_id, self.review_timeout)
            .await;

        let response = match wait_result {
            Ok(r) => {
                span.record("timed_out", false);
                r
            }
            Err(e) => {
                span.record("timed_out", true);
                span.record("decision", "timeout");
                return Err(GlobalError::Other(format!(
                    "HITL wait_for_review failed for task '{}': {e}",
                    task.id
                )));
            }
        };

        match response {
            ReviewResponse::Approved { .. } => {
                span.record("decision", "approved");
                tracing::info!(task_id = %task.id, "hitl.decision" = "approved");
                Ok(None)
            }
            ReviewResponse::Rejected { reason, .. } => {
                span.record("decision", "rejected");
                tracing::warn!(task_id = %task.id, "hitl.decision" = "rejected", %reason);
                Err(GlobalError::Other(format!(
                    "Task '{}' rejected by reviewer: {reason}",
                    task.id
                )))
            }
            ReviewResponse::ChangesRequested { changes, .. } => {
                span.record("decision", "changes_requested");
                tracing::info!(task_id = %task.id, "hitl.decision" = "changes_requested");
                let modified = format!(
                    "## Reviewer Requested Changes\n{}\n\n{}",
                    changes, task.description
                );
                Ok(Some(modified))
            }
            _ => {
                span.record("decision", "deferred");
                tracing::warn!(task_id = %task.id, "hitl.decision" = "deferred");
                Err(GlobalError::Other(format!(
                    "Task '{}' deferred — not approved for execution",
                    task.id
                )))
            }
        }
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hitl::manager::{ReviewManager, ReviewManagerConfig};
    use crate::hitl::notifier::ReviewNotifier;
    use crate::hitl::policy_engine::ReviewPolicyEngine;
    use crate::hitl::store::InMemoryReviewStore;
    use crate::swarm::dag::{RiskLevel, SubtaskDAG, SwarmSubtask};
    use crate::swarm::scheduler::{
        FailurePolicy, ParallelScheduler, SequentialScheduler, SwarmScheduler,
        SwarmSchedulerConfig, TaskOutcome,
    };
    use mofa_kernel::hitl::ReviewResponse;
    use std::sync::Arc;
    use std::time::Duration;

    /// Build a minimal `ReviewManager` backed by an in-memory store.
    fn make_manager() -> Arc<ReviewManager> {
        let store = Arc::new(InMemoryReviewStore::new());
        let notifier = Arc::new(ReviewNotifier::default());
        let policy_engine = Arc::new(ReviewPolicyEngine::default());
        Arc::new(ReviewManager::new(
            store,
            notifier,
            policy_engine,
            None,
            ReviewManagerConfig::default(),
        ))
    }

    /// A simple executor that just returns the task id as output.
    fn echo_executor() -> SubtaskExecutorFn {
        Arc::new(|_idx, task: SwarmSubtask| {
            Box::pin(async move { Ok(format!("{}-done", task.id)) })
        })
    }

    // ── Test 1: HITLMode::None bypasses even Critical tasks ───────────────

    #[tokio::test]
    async fn test_gate_none_mode_bypasses_all_tasks() {
        let manager = make_manager();
        let gate = Arc::new(SwarmHITLGate::new(
            manager.clone(),
            HITLMode::None,
            "exec-1",
        ));
        let executor = gate.wrap_executor(echo_executor());

        let mut dag = SubtaskDAG::new("none-mode");
        let idx = dag.add_task(
            SwarmSubtask::new("critical-task", "Delete everything")
                .with_risk_level(RiskLevel::Critical),
        );

        let summary = SequentialScheduler::with_config(SwarmSchedulerConfig::default())
            .execute(&mut dag, executor)
            .await
            .unwrap();

        assert_eq!(summary.succeeded, 1);
        assert_eq!(summary.failed, 0);
        // No review requests should have been created.
        let pending = manager.list_pending(None, None).await.unwrap();
        assert!(pending.is_empty(), "HITLMode::None must never submit reviews");

        let _ = idx;
    }

    // ── Test 2: HITLMode::Required intercepts even Low-risk tasks ─────────

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn test_gate_required_mode_intercepts_low_risk() {
        let manager = make_manager();
        let gate = Arc::new(SwarmHITLGate::new(
            Arc::clone(&manager),
            HITLMode::Required,
            "exec-2",
        ));
        let executor = gate.wrap_executor(echo_executor());

        let mut dag = SubtaskDAG::new("required-mode");
        dag.add_task(SwarmSubtask::new("low-task", "Search the web").with_risk_level(RiskLevel::Low));

        // Poll until the review appears then approve it.
        let mgr = Arc::clone(&manager);
        tokio::spawn(async move {
            let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
            loop {
                let pending = mgr.list_pending(None, None).await.unwrap();
                if !pending.is_empty() {
                    for r in pending {
                        mgr.resolve_review(
                            &r.id,
                            ReviewResponse::Approved { comment: None },
                            "auto-approver".to_string(),
                        )
                        .await
                        .unwrap();
                    }
                    break;
                }
                if tokio::time::Instant::now() >= deadline {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        });

        let cfg = SwarmSchedulerConfig::default();
        let summary = SequentialScheduler::with_config(cfg)
            .execute(&mut dag, executor)
            .await
            .unwrap();

        assert_eq!(summary.succeeded, 1, "Low-risk task must succeed after approval");
    }

    // ── Test 3: HITLMode::Optional intercepts High-risk tasks ─────────────

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn test_gate_optional_mode_intercepts_high_risk() {
        let manager = make_manager();
        let gate = Arc::new(SwarmHITLGate::new(
            Arc::clone(&manager),
            HITLMode::Optional,
            "exec-3",
        ));
        let executor = gate.wrap_executor(echo_executor());

        let mut dag = SubtaskDAG::new("optional-high");
        dag.add_task(
            SwarmSubtask::new("high-task", "Update production database")
                .with_risk_level(RiskLevel::High),
        );

        // Poll until the review appears then approve it.
        let mgr = Arc::clone(&manager);
        tokio::spawn(async move {
            let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
            loop {
                let pending = mgr.list_pending(None, None).await.unwrap();
                if !pending.is_empty() {
                    for r in pending {
                        mgr.resolve_review(
                            &r.id,
                            ReviewResponse::Approved { comment: Some("LGTM".to_string()) },
                            "reviewer".to_string(),
                        )
                        .await
                        .unwrap();
                    }
                    break;
                }
                if tokio::time::Instant::now() >= deadline {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        });

        let cfg = SwarmSchedulerConfig::default();
        let summary = SequentialScheduler::with_config(cfg)
            .execute(&mut dag, executor)
            .await
            .unwrap();

        assert_eq!(summary.succeeded, 1);
    }

    // ── Test 4: HITLMode::Optional passes Low-risk tasks through directly ──

    #[tokio::test]
    async fn test_gate_optional_mode_passes_low_risk_direct() {
        let manager = make_manager();
        let gate = Arc::new(SwarmHITLGate::new(
            Arc::clone(&manager),
            HITLMode::Optional,
            "exec-4",
        ));
        let executor = gate.wrap_executor(echo_executor());

        let mut dag = SubtaskDAG::new("optional-low");
        dag.add_task(SwarmSubtask::new("low-task", "Search the web").with_risk_level(RiskLevel::Low));

        let cfg = SwarmSchedulerConfig::default();
        let summary = SequentialScheduler::with_config(cfg)
            .execute(&mut dag, executor)
            .await
            .unwrap();

        assert_eq!(summary.succeeded, 1);
        // No review created for a Low-risk task in Optional mode.
        let pending = manager.list_pending(None, None).await.unwrap();
        assert!(pending.is_empty());
    }

    // ── Test 5: Rejected review causes task failure ────────────────────────

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn test_gate_rejects_task_on_reviewer_rejection() {
        let manager = make_manager();
        let gate = Arc::new(SwarmHITLGate::new(
            Arc::clone(&manager),
            HITLMode::Required,
            "exec-5",
        ));
        let executor = gate.wrap_executor(echo_executor());

        let mut dag = SubtaskDAG::new("rejection");
        dag.add_task(
            SwarmSubtask::new("risky-task", "Deploy to production")
                .with_risk_level(RiskLevel::Critical),
        );

        // Poll until the review appears then reject it.
        let mgr = Arc::clone(&manager);
        tokio::spawn(async move {
            let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
            loop {
                let pending = mgr.list_pending(None, None).await.unwrap();
                if !pending.is_empty() {
                    for r in pending {
                        mgr.resolve_review(
                            &r.id,
                            ReviewResponse::Rejected {
                                reason: "Not safe to deploy now".to_string(),
                                comment: None,
                            },
                            "reviewer".to_string(),
                        )
                        .await
                        .unwrap();
                    }
                    break;
                }
                if tokio::time::Instant::now() >= deadline {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        });

        let cfg = SwarmSchedulerConfig {
            failure_policy: FailurePolicy::Continue,
            ..Default::default()
        };
        let summary = SequentialScheduler::with_config(cfg)
            .execute(&mut dag, executor)
            .await
            .unwrap();

        assert_eq!(summary.succeeded, 0);
        assert_eq!(summary.failed, 1, "Rejected task must be marked Failed");

        // Verify the failure reason contains the rejection text.
        let result = &summary.results[0];
        if let TaskOutcome::Failure(reason) = &result.outcome {
            assert!(
                reason.contains("Not safe to deploy now"),
                "failure reason must include reviewer message: {reason}"
            );
        } else {
            panic!("Expected Failure outcome, got: {:?}", result.outcome);
        }
    }

    // ── Test 6: ChangesRequested applies modified description before execution

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn test_gate_changes_requested_modifies_task_description() {
        let manager = make_manager();
        let gate = Arc::new(SwarmHITLGate::new(
            Arc::clone(&manager),
            HITLMode::Required,
            "exec-cr",
        ));

        // Executor that captures the description it received.
        let captured_desc = Arc::new(std::sync::Mutex::new(String::new()));
        let captured = Arc::clone(&captured_desc);
        let capturing_executor: SubtaskExecutorFn = Arc::new(move |_idx, task: SwarmSubtask| {
            let captured = Arc::clone(&captured);
            Box::pin(async move {
                *captured.lock().unwrap() = task.description.clone();
                Ok(format!("{}-done", task.id))
            })
        });

        let executor = gate.wrap_executor(capturing_executor);

        let mut dag = SubtaskDAG::new("changes-requested");
        dag.add_task(
            SwarmSubtask::new("cr-task", "Original description").with_risk_level(RiskLevel::High),
        );

        // Resolve with ChangesRequested.
        let mgr = Arc::clone(&manager);
        tokio::spawn(async move {
            let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
            loop {
                let pending = mgr.list_pending(None, None).await.unwrap();
                if !pending.is_empty() {
                    for r in pending {
                        mgr.resolve_review(
                            &r.id,
                            ReviewResponse::ChangesRequested {
                                changes: "Please add safety checks".to_string(),
                                comment: None,
                            },
                            "reviewer".to_string(),
                        )
                        .await
                        .unwrap();
                    }
                    break;
                }
                if tokio::time::Instant::now() >= deadline {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        });

        let summary = SequentialScheduler::with_config(SwarmSchedulerConfig::default())
            .execute(&mut dag, executor)
            .await
            .unwrap();

        assert_eq!(summary.succeeded, 1);
        let desc = captured_desc.lock().unwrap().clone();
        assert!(
            desc.contains("Please add safety checks"),
            "modified description must include reviewer changes: {desc}"
        );
        assert!(
            desc.contains("Original description"),
            "modified description must still contain original task: {desc}"
        );
    }

    // ── Test 7: Optional mode auto-approves when review times out ─────────

    #[tokio::test]
    async fn test_gate_optional_mode_auto_approves_on_timeout() {
        let manager = make_manager();
        let gate = Arc::new(
            SwarmHITLGate::new(Arc::clone(&manager), HITLMode::Optional, "exec-timeout")
                .with_review_timeout(Duration::from_millis(50)), // very short timeout
        );
        let executor = gate.wrap_executor(echo_executor());

        let mut dag = SubtaskDAG::new("optional-timeout");
        dag.add_task(
            SwarmSubtask::new("timeout-task", "High risk but optional gate")
                .with_risk_level(RiskLevel::High),
        );

        // Nobody approves — the gate times out. In Optional mode, should auto-approve.
        let summary = SequentialScheduler::with_config(SwarmSchedulerConfig::default())
            .execute(&mut dag, executor)
            .await
            .unwrap();

        assert_eq!(
            summary.succeeded, 1,
            "Optional gate timeout must auto-approve, not fail the task"
        );
    }

    // ── Test 8: Gate works with ParallelScheduler on a diamond DAG ─────────

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn test_gate_works_with_parallel_scheduler() {
        // fetch(Low) → {analyze_a(Low), analyze_b(Low)} → merge(High)
        let manager = make_manager();
        let gate = Arc::new(SwarmHITLGate::new(
            Arc::clone(&manager),
            HITLMode::Optional,
            "exec-6",
        ));
        let executor = gate.wrap_executor(echo_executor());

        let mut dag = SubtaskDAG::new("parallel-hitl");
        let fetch = dag.add_task(
            SwarmSubtask::new("fetch", "Fetch data").with_risk_level(RiskLevel::Low),
        );
        let analyze_a = dag.add_task(
            SwarmSubtask::new("analyze-a", "Analyse branch A").with_risk_level(RiskLevel::Low),
        );
        let analyze_b = dag.add_task(
            SwarmSubtask::new("analyze-b", "Analyse branch B").with_risk_level(RiskLevel::Low),
        );
        let merge = dag.add_task(
            SwarmSubtask::new("merge", "Merge and push results").with_risk_level(RiskLevel::High),
        );
        dag.add_dependency(fetch, analyze_a).unwrap();
        dag.add_dependency(fetch, analyze_b).unwrap();
        dag.add_dependency(analyze_a, merge).unwrap();
        dag.add_dependency(analyze_b, merge).unwrap();

        // Poll until the merge node's review appears then approve it.
        let mgr = Arc::clone(&manager);
        tokio::spawn(async move {
            let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
            loop {
                let pending = mgr.list_pending(None, None).await.unwrap();
                if !pending.is_empty() {
                    for r in pending {
                        mgr.resolve_review(
                            &r.id,
                            ReviewResponse::Approved { comment: None },
                            "auto-approver".to_string(),
                        )
                        .await
                        .unwrap();
                    }
                    break;
                }
                if tokio::time::Instant::now() >= deadline {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        });

        let cfg = SwarmSchedulerConfig::default();
        let summary = ParallelScheduler::with_config(cfg)
            .execute(&mut dag, executor)
            .await
            .unwrap();

        assert_eq!(summary.succeeded, 4, "all 4 tasks must succeed");
        assert_eq!(summary.failed, 0);
    }
}
