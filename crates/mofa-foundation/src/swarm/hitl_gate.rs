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
//!   Approved?        ──Yes──────────────────────► inner executor
//!   ChangesRequested ──apply changes, then──────► inner executor
//!   Optional timeout ──auto-approve, warn, then─► inner executor
//!   Rejected / Err   ──────────────────────────► Err (task Failed)
//! ```
//!
//! The returned [`SubtaskExecutorFn`] is a drop-in replacement that can be
//! passed directly to either scheduler.  After execution, call
//! [`SwarmHITLGate::enrich_summary`] to attach gate metrics to the
//! [`SchedulerSummary`].

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use mofa_kernel::agent::types::error::{GlobalError, GlobalResult};
use mofa_kernel::hitl::{
    ExecutionStep, ExecutionTrace, ReviewContext, ReviewRequest, ReviewResponse, ReviewType,
};
use serde::{Deserialize, Serialize};
use tracing::{Instrument, info_span};

use crate::hitl::manager::ReviewManager;
use crate::swarm::config::HITLMode;
use crate::swarm::dag::{RiskLevel, SwarmSubtask};
use crate::swarm::scheduler::{SchedulerSummary, SubtaskExecutorFn};

// ── Notifier ──────────────────────────────────────────────────────────────────

/// The outcome of a single reviewer decision.
///
/// Passed to [`HITLNotifier::on_decision`] so notifier implementations can
/// fan out decisions to Slack, PagerDuty, a CLI prompt, or any other sink
/// without touching the approval flow or replacing `ReviewManager`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub enum HITLDecision {
    /// Task approved as-is.
    Approved,
    /// Reviewer provided a modified description; task runs with the changes.
    Modified,
    /// Task rejected; execution will not proceed.
    Rejected,
    /// No reviewer responded before the timeout; auto-approved because the
    /// gate is in `HITLMode::Optional`.
    AutoApprovedTimeout,
}

/// Observer hook called on every gate interception and every reviewer decision.
///
/// Implement this trait to fan out events to any secondary sink — Slack,
/// PagerDuty, a CLI prompt, a metrics store — without changing the approval
/// flow or swapping out `ReviewManager`.
///
/// All methods have default no-op implementations so implementors only
/// override what they care about.
///
/// ```rust,ignore
/// struct SlackNotifier { webhook_url: String }
///
/// impl HITLNotifier for SlackNotifier {
///     fn on_decision(&self, task: &SwarmSubtask, decision: HITLDecision, latency_ms: u64) {
///         // post to Slack webhook
///     }
/// }
///
/// let gate = SwarmHITLGate::new(manager, HITLMode::Optional, "exec-1")
///     .with_notifier(Arc::new(SlackNotifier { webhook_url: "...".into() }));
/// ```
pub trait HITLNotifier: Send + Sync {
    /// Called immediately before a task is submitted for review.
    fn on_intercepted(&self, _task: &SwarmSubtask) {}

    /// Called after the reviewer decision is recorded.
    ///
    /// `latency_ms` is the round-trip time from `request_review` to the
    /// resolved response (or timeout).
    fn on_decision(&self, _task: &SwarmSubtask, _decision: HITLDecision, _latency_ms: u64) {}
}

// ── Metrics ───────────────────────────────────────────────────────────────────

/// Atomic counters used internally to track gate activity across concurrent tasks.
///
/// Uses `Relaxed` ordering throughout — these are independent per-field counters
/// with no cross-field synchronisation requirement.
#[derive(Debug, Default)]
struct MetricsInner {
    intercepted: AtomicU64,
    approved: AtomicU64,
    modified: AtomicU64,
    rejected: AtomicU64,
    auto_approved_timeout: AtomicU64,
    total_review_latency_ms: AtomicU64,
}

impl MetricsInner {
    fn snapshot(&self) -> HITLGateMetrics {
        HITLGateMetrics {
            intercepted: self.intercepted.load(Ordering::Relaxed),
            approved: self.approved.load(Ordering::Relaxed),
            modified: self.modified.load(Ordering::Relaxed),
            rejected: self.rejected.load(Ordering::Relaxed),
            auto_approved_timeout: self.auto_approved_timeout.load(Ordering::Relaxed),
            total_review_latency_ms: self.total_review_latency_ms.load(Ordering::Relaxed),
        }
    }
}

/// Point-in-time snapshot of gate activity for a single swarm execution.
///
/// Obtain one via [`SwarmHITLGate::metrics`] or by calling
/// [`SwarmHITLGate::enrich_summary`] to embed it directly in a
/// [`SchedulerSummary`].
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HITLGateMetrics {
    /// Tasks that passed through the interception check (submitted for review).
    pub intercepted: u64,
    /// Tasks approved as-is by a reviewer.
    pub approved: u64,
    /// Tasks where a reviewer requested changes; the modified description was
    /// applied before the task executed.
    pub modified: u64,
    /// Tasks rejected by a reviewer; each rejection causes a task `Failure`.
    pub rejected: u64,
    /// Tasks auto-approved after a timeout while in `HITLMode::Optional`.
    pub auto_approved_timeout: u64,
    /// Sum of all reviewer round-trip latencies in milliseconds.
    /// Divide by `approved + modified + rejected` for the mean latency.
    pub total_review_latency_ms: u64,
}

impl HITLGateMetrics {
    /// Mean reviewer round-trip latency in milliseconds.
    ///
    /// Returns `0` when no reviews have completed (avoids division by zero).
    pub fn avg_review_latency_ms(&self) -> u64 {
        let reviewed = self.approved + self.modified + self.rejected;
        if reviewed == 0 {
            0
        } else {
            self.total_review_latency_ms / reviewed
        }
    }
}

// ── Gate ──────────────────────────────────────────────────────────────────────

/// Gate that intercepts HITL-flagged subtasks before execution.
///
/// Construct one via [`SwarmHITLGate::new`], configure with the builder
/// methods, wrap your executor with [`SwarmHITLGate::wrap_executor`], and pass
/// the result to a scheduler.
///
/// ```rust,ignore
/// let gate = Arc::new(
///     SwarmHITLGate::new(manager, HITLMode::Optional, dag.id.clone())
///         .with_risk_threshold(RiskLevel::High)
///         .with_review_timeout(Duration::from_secs(300)),
/// );
/// let gated = gate.wrap_executor(inner_executor);
/// let summary = ParallelScheduler::new().execute(&mut dag, gated).await?;
/// let summary = gate.enrich_summary(summary);
/// println!("{:?}", summary.hitl_stats);
/// ```
pub struct SwarmHITLGate {
    manager: Arc<ReviewManager>,
    mode: HITLMode,
    execution_id: String,
    /// Minimum risk level that triggers a review in `HITLMode::Optional`.
    /// Defaults to [`RiskLevel::High`].
    risk_threshold: RiskLevel,
    /// Maximum time to wait for a reviewer decision.
    /// `None` delegates to the `ReviewManager`'s configured expiration (1 h).
    review_timeout: Option<Duration>,
    /// Optional custom interception predicate.
    ///
    /// When set, replaces the built-in `risk_threshold` / `mode` logic.
    /// Use [`with_intercept_when`] to install one.
    intercept_when: Option<Arc<dyn Fn(&SwarmSubtask) -> bool + Send + Sync>>,
    /// Optional observer notified on every interception and decision.
    notifier: Option<Arc<dyn HITLNotifier>>,
    /// Shared atomic counters written by every task closure concurrently.
    metrics: Arc<MetricsInner>,
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
            intercept_when: None,
            notifier: None,
            metrics: Arc::new(MetricsInner::default()),
        }
    }

    /// Override the minimum risk level that triggers a review.
    ///
    /// Only applies when `mode` is [`HITLMode::Optional`] and no custom
    /// `intercept_when` predicate has been installed.
    pub fn with_risk_threshold(mut self, threshold: RiskLevel) -> Self {
        self.risk_threshold = threshold;
        self
    }

    /// Override the maximum wait time for reviewer decisions.
    pub fn with_review_timeout(mut self, timeout: Duration) -> Self {
        self.review_timeout = Some(timeout);
        self
    }

    /// Install a custom interception predicate.
    ///
    /// When set, the predicate **replaces** the built-in
    /// `risk_threshold` / `HITLMode` check entirely.  Use this to intercept
    /// on arbitrary task properties — capability requirements, description
    /// keywords, estimated duration, etc.
    ///
    /// ```rust,ignore
    /// let gate = SwarmHITLGate::new(manager, HITLMode::Optional, "exec-1")
    ///     .with_intercept_when(|task| {
    ///         task.required_capabilities.contains(&"write_db".to_string())
    ///             || task.estimated_duration_secs.unwrap_or(0) > 300
    ///     });
    /// ```
    pub fn with_intercept_when(
        mut self,
        predicate: impl Fn(&SwarmSubtask) -> bool + Send + Sync + 'static,
    ) -> Self {
        self.intercept_when = Some(Arc::new(predicate));
        self
    }

    /// Attach an [`HITLNotifier`] observer to the gate.
    ///
    /// The notifier receives every interception and every reviewer decision.
    /// Multiple notifiers can be composed behind a single wrapper struct.
    /// The notifier does not affect the approval flow — it is called
    /// after the gate has already recorded the decision internally.
    pub fn with_notifier(mut self, notifier: Arc<dyn HITLNotifier>) -> Self {
        self.notifier = Some(notifier);
        self
    }

    /// Returns a snapshot of gate activity metrics.
    ///
    /// Safe to call at any time, including concurrently with ongoing execution.
    /// For the final summary, prefer [`enrich_summary`] instead.
    pub fn metrics(&self) -> HITLGateMetrics {
        self.metrics.snapshot()
    }

    /// Attach the gate's metrics to a [`SchedulerSummary`].
    ///
    /// Call this immediately after the scheduler returns, before the summary
    /// is logged or returned to the caller:
    ///
    /// ```rust,ignore
    /// let summary = scheduler.execute(&mut dag, gated).await?;
    /// let summary = gate.enrich_summary(summary);
    /// ```
    pub fn enrich_summary(&self, mut summary: SchedulerSummary) -> SchedulerSummary {
        summary.hitl_stats = Some(self.metrics.snapshot());
        summary
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
                        gate.metrics.intercepted.fetch_add(1, Ordering::Relaxed);
                        if let Some(ref n) = gate.notifier {
                            n.on_intercepted(&task_for_gate);
                        }

                        let gate_span = info_span!(
                            "hitl.approval_gate",
                            review_id = tracing::field::Empty,
                            decision = tracing::field::Empty,
                            timed_out = tracing::field::Empty,
                        );

                        let review_start = std::time::Instant::now();
                        let result = gate
                            .request_and_wait(&task_for_gate, &gate_span)
                            .instrument(gate_span.clone())
                            .await;
                        let latency_ms =
                            u64::try_from(review_start.elapsed().as_millis()).unwrap_or(u64::MAX);

                        match result {
                            Ok(None) => {
                                gate.metrics.approved.fetch_add(1, Ordering::Relaxed);
                                gate.metrics
                                    .total_review_latency_ms
                                    .fetch_add(latency_ms, Ordering::Relaxed);
                                if let Some(ref n) = gate.notifier {
                                    n.on_decision(
                                        &task_for_gate,
                                        HITLDecision::Approved,
                                        latency_ms,
                                    );
                                }
                            }
                            Ok(Some(modified_desc)) => {
                                gate.metrics.modified.fetch_add(1, Ordering::Relaxed);
                                gate.metrics
                                    .total_review_latency_ms
                                    .fetch_add(latency_ms, Ordering::Relaxed);
                                if let Some(ref n) = gate.notifier {
                                    n.on_decision(
                                        &task_for_gate,
                                        HITLDecision::Modified,
                                        latency_ms,
                                    );
                                }
                                task.description = modified_desc;
                            }
                            Err(e) if gate.is_optional() => {
                                gate.metrics
                                    .auto_approved_timeout
                                    .fetch_add(1, Ordering::Relaxed);
                                tracing::warn!(
                                    task_id = %task_for_gate.id,
                                    "hitl timeout in Optional mode — auto-approving"
                                );
                                if let Some(ref n) = gate.notifier {
                                    n.on_decision(
                                        &task_for_gate,
                                        HITLDecision::AutoApprovedTimeout,
                                        latency_ms,
                                    );
                                }
                                let _ = e;
                            }
                            Err(e) => {
                                gate.metrics.rejected.fetch_add(1, Ordering::Relaxed);
                                gate.metrics
                                    .total_review_latency_ms
                                    .fetch_add(latency_ms, Ordering::Relaxed);
                                if let Some(ref n) = gate.notifier {
                                    n.on_decision(
                                        &task_for_gate,
                                        HITLDecision::Rejected,
                                        latency_ms,
                                    );
                                }
                                return Err(e);
                            }
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

    // ── Private ───────────────────────────────────────────────────────────

    fn is_optional(&self) -> bool {
        matches!(self.mode, HITLMode::Optional)
    }

    /// Determine whether this task must be reviewed before execution.
    ///
    /// If a custom predicate was installed via [`with_intercept_when`], it
    /// takes full precedence and the `mode` / `risk_threshold` fields are
    /// ignored.
    fn should_intercept(&self, task: &SwarmSubtask) -> bool {
        if let Some(ref predicate) = self.intercept_when {
            return predicate(task);
        }
        match self.mode {
            HITLMode::None => false,
            HITLMode::Required => true,
            HITLMode::Optional => task.hitl_required || task.risk_level >= self.risk_threshold,
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
        let now_ms = u64::try_from(chrono::Utc::now().timestamp_millis()).unwrap_or(u64::MAX);

        let trace = ExecutionTrace {
            steps: vec![ExecutionStep {
                step_id: task.id.clone(),
                step_type: "swarm_subtask".to_string(),
                timestamp_ms: now_ms,
                input: Some(serde_json::json!({
                    "description":             task.description,
                    "risk_level":              format!("{:?}", task.risk_level),
                    "complexity":              task.complexity,
                    "required_capabilities":   task.required_capabilities,
                    "estimated_duration_secs": task.estimated_duration_secs,
                })),
                output: None,
                metadata: Default::default(),
            }],
            duration_ms: task
                .estimated_duration_secs
                .unwrap_or(0)
                .saturating_mul(1_000),
        };

        let context = ReviewContext::new(
            trace,
            serde_json::json!({
                "task_id":                 task.id,
                "description":             task.description,
                "risk_level":              format!("{:?}", task.risk_level),
                "estimated_duration_secs": task.estimated_duration_secs,
                "required_capabilities":   task.required_capabilities,
            }),
        );

        let mut request = ReviewRequest::new(&self.execution_id, ReviewType::Approval, context)
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
