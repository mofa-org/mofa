//! HITL gate — intercepts task execution and requests human approval.
//!
//! ## How it fits into the scheduler
//!
//! ```text
//!  SequentialScheduler / ParallelScheduler
//!        │
//!        │  for each task
//!        ▼
//!   task.hitl_required?
//!     │ yes                      no
//!     ▼                          │
//!  HITLGate::await_decision      │
//!     │                          │
//!   Approved ─────────────────►  execute task
//!   Rejected ──────────────────► mark failed, cascade skip if policy
//!   TimedOut ──────────────────► mark failed
//! ```
//!
//! `SwarmHITLGate` is the production implementation backed by `ReviewManager`.
//! Tests can provide any `impl HITLGate` via `SwarmSchedulerConfig::with_hitl_gate`.

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;

use crate::hitl::manager::ReviewManager;
use crate::swarm::dag::SwarmSubtask;
use mofa_kernel::hitl::context::{ExecutionTrace, ReviewContext};
use mofa_kernel::hitl::types::{ReviewMetadata, ReviewRequest, ReviewResponse, ReviewType};

/// outcome of a human review for a swarm task
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum HITLDecision {
    /// human approved — proceed with execution
    Approved,
    /// human rejected — skip task with this reason
    Rejected(String),
    /// no response before deadline — treat as failure
    TimedOut,
}

impl HITLDecision {
    pub fn is_approved(&self) -> bool {
        matches!(self, Self::Approved)
    }
}

/// gate that intercepts task execution and requests human approval
#[async_trait]
pub trait HITLGate: Send + Sync {
    /// returns true if this task must be reviewed before execution
    fn requires_review(&self, task: &SwarmSubtask) -> bool;

    /// submit a review and block until a decision is received or timeout expires
    async fn await_decision(
        &self,
        task: &SwarmSubtask,
        execution_id: &str,
        timeout: Duration,
    ) -> HITLDecision;
}

/// Production HITL gate backed by `ReviewManager`.
///
/// Submits a review request containing task metadata and risk classification,
/// then polls until a human responds or the timeout is reached.
pub struct SwarmHITLGate {
    manager: Arc<ReviewManager>,
}

impl SwarmHITLGate {
    pub fn new(manager: Arc<ReviewManager>) -> Self {
        Self { manager }
    }
}

#[async_trait]
impl HITLGate for SwarmHITLGate {
    fn requires_review(&self, task: &SwarmSubtask) -> bool {
        task.hitl_required
    }

    async fn await_decision(
        &self,
        task: &SwarmSubtask,
        execution_id: &str,
        timeout: Duration,
    ) -> HITLDecision {
        let trace = ExecutionTrace {
            steps: vec![],
            duration_ms: 0,
        };

        // build context from task metadata
        let input = serde_json::json!({
            "task_id": &task.id,
            "description": &task.description,
            "risk_level": format!("{:?}", task.risk_level),
            "estimated_duration_secs": task.estimated_duration_secs,
        });

        let context = ReviewContext::new(trace, input);

        let mut metadata = ReviewMetadata::default();
        metadata.priority = task.risk_level.to_priority();
        metadata.tags = vec![
            "swarm".to_string(),
            format!("{:?}", task.risk_level).to_lowercase(),
        ];

        let mut request = ReviewRequest::new(execution_id, ReviewType::Approval, context);
        request.node_id = Some(task.id.clone());
        request.metadata = metadata;

        let review_id = match self.manager.request_review(request).await {
            Ok(id) => id,
            Err(e) => {
                tracing::warn!(
                    task_id = %task.id,
                    error = %e,
                    "hitl gate: failed to submit review"
                );
                return HITLDecision::Rejected(format!("review submission failed: {e}"));
            }
        };

        tracing::info!(
            task_id = %task.id,
            review_id = %review_id.as_str(),
            "hitl gate: awaiting human decision"
        );

        match self.manager.wait_for_review(&review_id, Some(timeout)).await {
            Ok(ReviewResponse::Approved { .. }) => HITLDecision::Approved,
            Ok(ReviewResponse::Rejected { reason, .. }) => HITLDecision::Rejected(reason),
            Ok(ReviewResponse::ChangesRequested { changes, .. }) => {
                HITLDecision::Rejected(format!("changes requested: {changes}"))
            }
            Ok(ReviewResponse::Deferred { reason }) => {
                HITLDecision::Rejected(format!("deferred: {reason}"))
            }
            Ok(_) => HITLDecision::Rejected("unrecognized review response".to_string()),
            Err(_) => HITLDecision::TimedOut,
        }
    }
}

/// minimal gate that auto-approves every task — for tests
#[cfg(test)]
pub struct AlwaysApproveGate;

#[cfg(test)]
#[async_trait]
impl HITLGate for AlwaysApproveGate {
    fn requires_review(&self, task: &SwarmSubtask) -> bool {
        task.hitl_required
    }

    async fn await_decision(
        &self,
        _task: &SwarmSubtask,
        _execution_id: &str,
        _timeout: Duration,
    ) -> HITLDecision {
        HITLDecision::Approved
    }
}

/// gate that rejects every task — for tests
#[cfg(test)]
pub struct AlwaysRejectGate;

#[cfg(test)]
#[async_trait]
impl HITLGate for AlwaysRejectGate {
    fn requires_review(&self, task: &SwarmSubtask) -> bool {
        task.hitl_required
    }

    async fn await_decision(
        &self,
        _task: &SwarmSubtask,
        _execution_id: &str,
        _timeout: Duration,
    ) -> HITLDecision {
        HITLDecision::Rejected("rejected by test gate".to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::swarm::dag::{RiskLevel, SwarmSubtask};

    #[test]
    fn decision_is_approved_helper() {
        assert!(HITLDecision::Approved.is_approved());
        assert!(!HITLDecision::Rejected("x".to_string()).is_approved());
        assert!(!HITLDecision::TimedOut.is_approved());
    }

    #[test]
    fn requires_review_follows_hitl_required_flag() {
        let gate = AlwaysApproveGate;
        let low_risk = SwarmSubtask::new("t1", "low risk task");
        let high_risk = SwarmSubtask::new("t2", "critical task")
            .with_risk_level(RiskLevel::Critical);

        assert!(!gate.requires_review(&low_risk));
        assert!(gate.requires_review(&high_risk));
    }

    #[tokio::test]
    async fn always_approve_gate_returns_approved() {
        let gate = AlwaysApproveGate;
        let task = SwarmSubtask::new("t1", "task").with_risk_level(RiskLevel::High);
        let decision = gate
            .await_decision(&task, "exec-1", Duration::from_secs(5))
            .await;
        assert_eq!(decision, HITLDecision::Approved);
    }

    #[tokio::test]
    async fn always_reject_gate_returns_rejected() {
        let gate = AlwaysRejectGate;
        let task = SwarmSubtask::new("t1", "task").with_risk_level(RiskLevel::High);
        let decision = gate
            .await_decision(&task, "exec-1", Duration::from_secs(5))
            .await;
        assert!(matches!(decision, HITLDecision::Rejected(_)));
    }
}
