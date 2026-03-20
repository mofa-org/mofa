//! HITL (Human in the Loop) approval workflow for swarm schedulers

use async_trait::async_trait;
use std::sync::Arc;
use tokio::sync::{mpsc, oneshot, Mutex};

use mofa_kernel::agent::types::error::GlobalResult;

use crate::swarm::config::{AuditEvent, AuditEventKind, HITLMode};
use crate::swarm::scheduler::SubtaskExecutorFn;

/// The human's decision types for a pending subtask
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum ApprovalDecision {
    Approve,
    Reject,
    Modify(String),
}

#[derive(Debug, Clone)]
pub struct ApprovalRequest {
    pub subtask_id: String,
    pub description: String,
    pub prior_output: Option<String>,
    pub risk_level: f64,
}

#[derive(Debug)]
pub struct ApprovalOutcome {
    pub decision: ApprovalDecision,
    pub reason: Option<String>,
}

impl ApprovalOutcome {
    pub fn approve() -> Self {
        Self { decision: ApprovalDecision::Approve, reason: None }
    }
    pub fn reject(reason: impl Into<String>) -> Self {
        Self { decision: ApprovalDecision::Reject, reason: Some(reason.into()) }
    }
    pub fn modify(new_prompt: impl Into<String>) -> Self {
        Self { decision: ApprovalDecision::Modify(new_prompt.into()), reason: None }
    }
}

/// Async interface for approval backends.
#[async_trait]
pub trait ApprovalHandler: Send + Sync {
    async fn request_approval(&self, req: ApprovalRequest) -> ApprovalOutcome;
}

type ApprovalMsg = (ApprovalRequest, oneshot::Sender<ApprovalOutcome>);

/// In process approval handler backed by an `mpsc` channel
pub struct ChannelApprovalHandler {
    tx: mpsc::Sender<ApprovalMsg>,
}

impl ChannelApprovalHandler {
    pub fn new(buffer: usize) -> (Self, mpsc::Receiver<ApprovalMsg>) {
        let (tx, rx) = mpsc::channel(buffer);
        (Self { tx }, rx)
    }
}

#[async_trait]
impl ApprovalHandler for ChannelApprovalHandler {
    async fn request_approval(&self, req: ApprovalRequest) -> ApprovalOutcome {
        let (reply_tx, reply_rx) = oneshot::channel();
        // If the receiver is gone, default to Approve so the scheduler doesn't deadlock.
        if self.tx.send((req, reply_tx)).await.is_err() {
            return ApprovalOutcome::approve();
        }
        reply_rx.await.unwrap_or_else(|_| ApprovalOutcome::approve())
    }
}

/// A middleware that wraps any `SubtaskExecutorFn` with HITL gates.
/// This allows HITL to work automatically inside Sequential, Parallel, or any Custom scheduler.
pub fn hitl_executor_middleware(
    base_executor: SubtaskExecutorFn,
    hitl_mode: HITLMode,
    handler: Arc<dyn ApprovalHandler>,
    audit_log: Arc<Mutex<Vec<AuditEvent>>>,
    optional_timeout: std::time::Duration,
) -> SubtaskExecutorFn {
    Arc::new(move |idx, mut task| {
        let base = base_executor.clone();
        let handler = handler.clone();
        let hitl_mode = hitl_mode.clone();
        let audit = audit_log.clone();
        let optional_timeout = optional_timeout;

        Box::pin(async move {
            let id = task.id.clone();
            let desc = task.description.clone();
            let complexity = task.complexity;

            // Prior output might be available if passed down through task definitions or context, 
            // but for raw DAG execution here, we just surface the node ID and description.
            let prior_output = None;

            let modified_task = match hitl_mode {
                HITLMode::None => task,
                HITLMode::Required => {
                    let req = ApprovalRequest {
                        subtask_id: id.clone(),
                        description: desc.clone(),
                        prior_output: prior_output.clone(),
                        risk_level: complexity,
                    };

                    audit.lock().await.push(AuditEvent::new(
                        AuditEventKind::HITLRequested,
                        format!("Approval requested for subtask '{id}'"),
                    ).with_data(serde_json::json!({ "subtask_id": id, "risk": complexity })));

                    let outcome = handler.request_approval(req).await;

                    let (decision_label, modified_desc) = match &outcome.decision {
                        ApprovalDecision::Approve => ("approved".to_string(), desc.clone()),
                        ApprovalDecision::Modify(p) => ("modified".to_string(), p.clone()),
                        ApprovalDecision::Reject => {
                            audit.lock().await.push(AuditEvent::new(
                                AuditEventKind::HITLDecision,
                                format!("Subtask '{id}' rejected"),
                            ).with_data(serde_json::json!({
                                "subtask_id": id,
                                "decision": "reject",
                                "reason": outcome.reason,
                            })));
                            return Err(mofa_kernel::agent::types::error::GlobalError::Other(
                                format!("Subtask '{id}' rejected by reviewer")
                            ));
                        }
                    };

                    let mut data = serde_json::json!({ "subtask_id": id, "decision": decision_label });
                    if let ApprovalDecision::Modify(p) = &outcome.decision {
                        data["modified_prompt"] = serde_json::Value::String(p.clone());
                    }
                    if let Some(r) = &outcome.reason {
                        data["reason"] = serde_json::Value::String(r.clone());
                    }
                    audit.lock().await.push(AuditEvent::new(
                        AuditEventKind::HITLDecision,
                        format!("Subtask '{id}' {decision_label}"),
                    ).with_data(data));

                    task.description = modified_desc;
                    task
                }
                HITLMode::Optional => {
                    let req = ApprovalRequest {
                        subtask_id: id.clone(),
                        description: desc.clone(),
                        prior_output: prior_output.clone(),
                        risk_level: complexity,
                    };

                    audit.lock().await.push(AuditEvent::new(
                        AuditEventKind::HITLRequested,
                        format!("Optional approval requested for subtask '{id}'"),
                    ).with_data(serde_json::json!({ "subtask_id": id, "risk": complexity })));

                    let outcome_result = tokio::time::timeout(
                        optional_timeout,
                        handler.request_approval(req),
                    )
                    .await;

                    let (outcome, timed_out) = match outcome_result {
                        Ok(outcome) => (outcome, false),
                        Err(_) => (ApprovalOutcome::approve(), true),
                    };

                    let (decision_label, modified_desc) = match &outcome.decision {
                        ApprovalDecision::Approve => (
                            if timed_out { "auto-approved" } else { "approved" }.to_string(),
                            desc.clone(),
                        ),
                        ApprovalDecision::Modify(p) => ("modified".to_string(), p.clone()),
                        ApprovalDecision::Reject => {
                            audit.lock().await.push(AuditEvent::new(
                                AuditEventKind::HITLDecision,
                                format!("Subtask '{id}' rejected (optional gate)"),
                            ).with_data(serde_json::json!({
                                "subtask_id": id, "decision": "reject", "reason": outcome.reason,
                            })));
                            return Err(mofa_kernel::agent::types::error::GlobalError::Other(
                                format!("Subtask '{id}' rejected by reviewer")
                            ));
                        }
                    };

                    audit.lock().await.push(AuditEvent::new(
                        AuditEventKind::HITLDecision,
                        format!("Subtask '{id}' {decision_label}"),
                    ).with_data(serde_json::json!({ "subtask_id": id, "decision": decision_label })));

                    task.description = modified_desc;
                    task
                }
            };

            // Delegate to the actual execution logic
            base(idx, modified_task).await
        })
    })
}

/// An [`ApprovalHandler`] that delegates to the production [`ReviewManager`].
///
/// This bridges the lightweight swarm HITL abstraction with the full
/// production review infrastructure (audit trail, webhooks, REST API, rate
/// limiting)
///
/// # Usage
/// ```ignore
/// let handler = ReviewManagerApprovalHandler::new(manager, "swarm-run-42");
/// let executor = hitl_executor_middleware(base, HITLMode::Required, Arc::new(handler), audit, timeout);
/// ```
///
/// For tests and local dev, use [`ChannelApprovalHandler`] instead
/// it works fully in-process with no dependencies.
pub struct ReviewManagerApprovalHandler {
    manager: Arc<crate::hitl::manager::ReviewManager>,
    execution_id: String,
    review_timeout: std::time::Duration,
}

impl ReviewManagerApprovalHandler {
    pub fn new(
        manager: Arc<crate::hitl::manager::ReviewManager>,
        execution_id: impl Into<String>,
    ) -> Self {
        Self {
            manager,
            execution_id: execution_id.into(),
            review_timeout: std::time::Duration::from_secs(3600),
        }
    }

    /// Override the default 1-hour wait timeout.
    pub fn with_review_timeout(mut self, timeout: std::time::Duration) -> Self {
        self.review_timeout = timeout;
        self
    }
}

#[async_trait]
impl ApprovalHandler for ReviewManagerApprovalHandler {
    async fn request_approval(&self, req: ApprovalRequest) -> ApprovalOutcome {
        use mofa_kernel::hitl::{
            ExecutionTrace, ReviewContext, ReviewRequest, ReviewResponse, ReviewType,
        };

        // Build a minimal ReviewContext carrying the task description and risk level.
        let input_data = serde_json::json!({
            "subtask_id": req.subtask_id,
            "description": req.description,
            "risk_level": req.risk_level,
            "prior_output": req.prior_output,
        });
        let trace = ExecutionTrace { steps: vec![], duration_ms: 0 };
        let context = ReviewContext::new(trace, input_data);

        let review_req = ReviewRequest::new(
            self.execution_id.clone(),
            ReviewType::Approval,
            context,
        )
        .with_node_id(req.subtask_id.clone());

        // Submit to ReviewManager (fires webhooks, stores in ReviewStore, etc.).
        // Infra errors (storage down, misconfiguration) → reject: don't silently approve.
        let id = match self.manager.request_review(review_req).await {
            Ok(id) => id,
            Err(err) => return ApprovalOutcome::reject(format!("review request failed: {err}")),
        };

        // Block until the human resolves via REST API (or we time out).
        let response = match self.manager.wait_for_review(&id, Some(self.review_timeout)).await {
            Ok(r) => r,
            // Only timeout/expiry auto-approves; other infra errors reject.
            Err(err) => {
                let msg = err.to_string();
                if msg.contains("timed out") || msg.contains("expired") {
                    return ApprovalOutcome::approve();
                }
                return ApprovalOutcome::reject(format!("review wait failed: {msg}"));
            }
        };

        // Map ReviewResponse to ApprovalOutcome.
        match response {
            ReviewResponse::Approved { comment } => ApprovalOutcome {
                decision: ApprovalDecision::Approve,
                reason: comment,
            },
            ReviewResponse::Rejected { reason, .. } => ApprovalOutcome::reject(reason),
            ReviewResponse::ChangesRequested { changes, .. } => ApprovalOutcome::modify(changes),
            ReviewResponse::Deferred { .. } => ApprovalOutcome::approve(),
            _ => ApprovalOutcome::approve(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hitl::store::{ReviewStore, ReviewStoreError};
    use crate::swarm::dag::{SubtaskDAG, SwarmSubtask};
    use crate::swarm::scheduler::{SequentialScheduler, SwarmScheduler, SwarmSchedulerConfig};
    use async_trait::async_trait;
    use mofa_kernel::hitl::{ReviewRequest, ReviewResponse, ReviewStatus};
    use tokio::task;
    use uuid::Uuid;

    fn one_task_dag() -> SubtaskDAG {
        let mut d = SubtaskDAG::new("test");
        d.add_task(SwarmSubtask::new("t1", "Run the analysis"));
        d
    }

    fn mock_base_executor() -> SubtaskExecutorFn {
        Arc::new(|_idx, task| {
            Box::pin(async move { Ok(format!("mock output for {}", task.description)) })
        })
    }

    /// Reviewer receives the request and sends Approve, task completes.
    #[tokio::test]
    async fn test_hitl_channel_approve() {
        let (handler, mut rx) = ChannelApprovalHandler::new(4);
        let handler_arc = Arc::new(handler);

        let reviewer = task::spawn(async move {
            let (req, reply) = rx.recv().await.expect("expected approval request");
            assert_eq!(req.subtask_id, "t1");
            assert!(!req.description.is_empty());
            reply.send(ApprovalOutcome::approve()).ok();
        });

        let mut dag = one_task_dag();
        let audit_log = Arc::new(Mutex::new(vec![]));
        let config = SwarmSchedulerConfig::default();

        let executor = hitl_executor_middleware(
            mock_base_executor(),
            HITLMode::Required,
            handler_arc,
            audit_log,
            config.hitl_optional_timeout,
        );

        let scheduler = SequentialScheduler::new();
        let summary = scheduler.execute(&mut dag, executor).await.unwrap();

        reviewer.await.unwrap();
        assert_eq!(summary.succeeded, 1);
        assert_eq!(summary.failed, 0);
        assert!(summary.results[0].outcome.is_success());
    }

    /// Reviewer sends Reject, scheduler halts, downstream task stays Pending.
    /// Reviewer sends Reject, scheduler halts with FailFastCascade, downstream task is Skipped.
    #[tokio::test]
    async fn test_hitl_channel_reject() {
        let (handler, mut rx) = ChannelApprovalHandler::new(4);
        let handler_arc = Arc::new(handler);

        let mut dag = SubtaskDAG::new("chain");
        let a = dag.add_task(SwarmSubtask::new("a", "Step 1"));
        let b = dag.add_task(SwarmSubtask::new("b", "Step 2"));
        dag.add_dependency(a, b).unwrap();

        let reviewer = task::spawn(async move {
            let (req, reply) = rx.recv().await.expect("expected approval request");
            assert_eq!(req.subtask_id, "a");
            reply.send(ApprovalOutcome::reject("too risky")).ok();
        });

        let audit_log = Arc::new(Mutex::new(vec![]));
        let executor = hitl_executor_middleware(
            mock_base_executor(),
            HITLMode::Required,
            handler_arc,
            audit_log,
            SwarmSchedulerConfig::default().hitl_optional_timeout,
        );

        // Use FailFastCascade so that rejecting `a` skips all dependents (`b`).
        let config = crate::swarm::scheduler::SwarmSchedulerConfig {
            failure_policy: crate::swarm::scheduler::FailurePolicy::FailFastCascade,
            ..Default::default()
        };
        let scheduler = SequentialScheduler::with_config(config);
        let summary = scheduler.execute(&mut dag, executor).await.unwrap();

        reviewer.await.unwrap();
        assert_eq!(summary.failed, 1);
        assert_eq!(dag.get_task(b).unwrap().status, crate::swarm::SubtaskStatus::Skipped);
    }


    /// Reviewer sends Modify, task runs with the reviewer's revised prompt.
    #[tokio::test]
    async fn test_hitl_channel_modify_prompt() {
        let (handler, mut rx) = ChannelApprovalHandler::new(4);
        let handler_arc = Arc::new(handler);

        let reviewer = task::spawn(async move {
            let (req, reply) = rx.recv().await.expect("expected approval request");
            assert!(req.description.contains("Run the analysis"));
            reply.send(ApprovalOutcome::modify("Summarise only the key findings")).ok();
        });

        let mut dag = one_task_dag();
        let audit_log = Arc::new(Mutex::new(vec![]));
        let executor = hitl_executor_middleware(
            mock_base_executor(),
            HITLMode::Required,
            handler_arc,
            audit_log,
            SwarmSchedulerConfig::default().hitl_optional_timeout,
        );

        let scheduler = SequentialScheduler::new();
        let summary = scheduler.execute(&mut dag, executor).await.unwrap();

        reviewer.await.unwrap();
        assert_eq!(summary.succeeded, 1);
        let out = summary.results[0].outcome.output().unwrap();
        assert!(out.contains("Summarise only the key findings"), "output was: {}", out);
    }

    #[tokio::test]
    async fn test_hitl_none_skips_channel() {
        let (handler, rx) = ChannelApprovalHandler::new(4);
        drop(rx); // Ensure no receiver exists

        let mut dag = one_task_dag();
        let audit_log = Arc::new(Mutex::new(vec![]));
        let config = SwarmSchedulerConfig::default();
        let handler_arc = Arc::new(handler);

        let executor = hitl_executor_middleware(
            mock_base_executor(),
            HITLMode::None,
            handler_arc,
            audit_log,
            config.hitl_optional_timeout,
        );

        let scheduler = SequentialScheduler::new();
        let summary = scheduler.execute(&mut dag, executor).await.unwrap();

        assert_eq!(summary.succeeded, 1);
    }

    /// audit log must contain HITLRequested + HITLDecision
    #[tokio::test]
    async fn test_hitl_audit_events_recorded() {
        let (handler, mut rx) = ChannelApprovalHandler::new(4);
        let handler_arc = Arc::new(handler);

        let reviewer = task::spawn(async move {
            let (_, reply) = rx.recv().await.expect("expected approval request");
            reply.send(ApprovalOutcome::approve()).ok();
        });

        let mut dag = one_task_dag();
        let audit_log = Arc::new(Mutex::new(vec![]));
        
        let executor = hitl_executor_middleware(
            mock_base_executor(),
            HITLMode::Required,
            handler_arc,
            audit_log.clone(),
            SwarmSchedulerConfig::default().hitl_optional_timeout,
        );

        let scheduler = SequentialScheduler::new();
        scheduler.execute(&mut dag, executor).await.unwrap();

        reviewer.await.unwrap();
        let audit = audit_log.lock().await;
        let kinds: Vec<_> = audit.iter().map(|e| &e.kind).collect();
        assert!(kinds.contains(&&AuditEventKind::HITLRequested), "missing HITLRequested");
        assert!(kinds.contains(&&AuditEventKind::HITLDecision),  "missing HITLDecision");
    }

    fn make_review_manager() -> Arc<crate::hitl::manager::ReviewManager> {
        use crate::hitl::{
            manager::{ReviewManager, ReviewManagerConfig},
            notifier::ReviewNotifier,
            policy_engine::ReviewPolicyEngine,
            store::InMemoryReviewStore,
        };
        Arc::new(ReviewManager::new(
            Arc::new(InMemoryReviewStore::new()),
            Arc::new(ReviewNotifier::new(vec![])),
            Arc::new(ReviewPolicyEngine::new(vec![])),
            None, // no rate limiting in tests
            ReviewManagerConfig::default(),
        ))
    }

    enum FailingStoreMode {
        Create,
        Get,
    }

    struct FailingReviewStore {
        mode: FailingStoreMode,
    }

    #[async_trait]
    impl ReviewStore for FailingReviewStore {
        async fn create_review(&self, _request: &ReviewRequest) -> Result<(), ReviewStoreError> {
            match self.mode {
                FailingStoreMode::Create => Err(ReviewStoreError::Connection("create failed".into())),
                FailingStoreMode::Get => Ok(()),
            }
        }

        async fn get_review(
            &self,
            _id: &mofa_kernel::hitl::ReviewRequestId,
        ) -> Result<Option<ReviewRequest>, ReviewStoreError> {
            match self.mode {
                FailingStoreMode::Create => Ok(None),
                FailingStoreMode::Get => Err(ReviewStoreError::Query("get failed".into())),
            }
        }

        async fn update_review(
            &self,
            _id: &mofa_kernel::hitl::ReviewRequestId,
            _status: ReviewStatus,
            _response: Option<ReviewResponse>,
            _resolved_by: Option<String>,
        ) -> Result<(), ReviewStoreError> {
            Ok(())
        }

        async fn list_pending(
            &self,
            _tenant_id: Option<Uuid>,
            _limit: Option<u64>,
        ) -> Result<Vec<ReviewRequest>, ReviewStoreError> {
            Ok(vec![])
        }

        async fn list_by_execution(
            &self,
            _execution_id: &str,
        ) -> Result<Vec<ReviewRequest>, ReviewStoreError> {
            Ok(vec![])
        }

        async fn list_expired(&self) -> Result<Vec<ReviewRequest>, ReviewStoreError> {
            Ok(vec![])
        }

        async fn cleanup_old_reviews(
            &self,
            _before: chrono::DateTime<chrono::Utc>,
        ) -> Result<u64, ReviewStoreError> {
            Ok(0)
        }
    }

    fn make_review_manager_with_store(
        store: Arc<dyn ReviewStore>,
    ) -> Arc<crate::hitl::manager::ReviewManager> {
        use crate::hitl::{
            manager::{ReviewManager, ReviewManagerConfig},
            notifier::ReviewNotifier,
            policy_engine::ReviewPolicyEngine,
        };
        Arc::new(ReviewManager::new(
            store,
            Arc::new(ReviewNotifier::new(vec![])),
            Arc::new(ReviewPolicyEngine::new(vec![])),
            None,
            ReviewManagerConfig::default(),
        ))
    }

    #[tokio::test]
    async fn test_review_manager_handler_approve() {
        use mofa_kernel::hitl::ReviewResponse;

        let manager = make_review_manager();
        let mgr_clone = Arc::clone(&manager);

        let handler = Arc::new(
            ReviewManagerApprovalHandler::new(Arc::clone(&manager), "test-run-1")
                .with_review_timeout(std::time::Duration::from_secs(5)),
        );

        // Background resolver: waits for a pending review then approves it.
        let resolver = task::spawn(async move {
            loop {
                tokio::time::sleep(std::time::Duration::from_millis(50)).await;
                let pending = mgr_clone.list_pending(None, Some(1)).await.unwrap();
                if let Some(req) = pending.into_iter().next() {
                    mgr_clone
                        .resolve_review(
                            &req.id,
                            ReviewResponse::Approved { comment: Some("looks good".into()) },
                            "auto-approver".into(),
                        )
                        .await
                        .unwrap();
                    return;
                }
            }
        });

        let req = ApprovalRequest {
            subtask_id: "task-1".into(),
            description: "Deploy to production".into(),
            risk_level: 0.8,
            prior_output: None,
        };

        let outcome = handler.request_approval(req).await;
        resolver.await.unwrap();

        assert!(
            matches!(outcome.decision, ApprovalDecision::Approve),
            "expected Approve, got {:?}", outcome.decision
        );
        assert_eq!(outcome.reason.as_deref(), Some("looks good"));
    }

    #[tokio::test]
    async fn test_review_manager_handler_reject() {
        use mofa_kernel::hitl::ReviewResponse;

        let manager = make_review_manager();
        let mgr_clone = Arc::clone(&manager);

        let handler = Arc::new(
            ReviewManagerApprovalHandler::new(Arc::clone(&manager), "test-run-2")
                .with_review_timeout(std::time::Duration::from_secs(5)),
        );

        let resolver = task::spawn(async move {
            loop {
                tokio::time::sleep(std::time::Duration::from_millis(50)).await;
                let pending = mgr_clone.list_pending(None, Some(1)).await.unwrap();
                if let Some(req) = pending.into_iter().next() {
                    mgr_clone
                        .resolve_review(
                            &req.id,
                            ReviewResponse::Rejected {
                                reason: "too risky".into(),
                                comment: None,
                            },
                            "auto-reviewer".into(),
                        )
                        .await
                        .unwrap();
                    return;
                }
            }
        });

        let req = ApprovalRequest {
            subtask_id: "task-2".into(),
            description: "Drop the production database".into(),
            risk_level: 0.95,
            prior_output: None,
        };

        let outcome = handler.request_approval(req).await;
        resolver.await.unwrap();

        assert!(
            matches!(outcome.decision, ApprovalDecision::Reject),
            "expected Reject, got {:?}", outcome.decision
        );
        assert_eq!(outcome.reason.as_deref(), Some("too risky"));
    }

    #[tokio::test]
    async fn test_review_manager_handler_timeout_auto_approves() {
        let manager = make_review_manager();
        let handler = ReviewManagerApprovalHandler::new(manager, "test-run-timeout")
            .with_review_timeout(std::time::Duration::from_millis(10));

        let req = ApprovalRequest {
            subtask_id: "task-timeout".into(),
            description: "Wait for approval".into(),
            risk_level: 0.5,
            prior_output: None,
        };

        let outcome = handler.request_approval(req).await;

        assert!(
            matches!(outcome.decision, ApprovalDecision::Approve),
            "expected timeout to auto-approve, got {:?}",
            outcome.decision
        );
        assert!(outcome.reason.is_none());
    }

    #[tokio::test]
    async fn test_review_manager_handler_request_review_failure_rejects() {
        let manager = make_review_manager_with_store(Arc::new(FailingReviewStore {
            mode: FailingStoreMode::Create,
        }));
        let handler = ReviewManagerApprovalHandler::new(manager, "test-run-create-fail");

        let outcome = handler
            .request_approval(ApprovalRequest {
                subtask_id: "task-create-fail".into(),
                description: "Create review should fail".into(),
                risk_level: 0.4,
                prior_output: None,
            })
            .await;

        assert!(
            matches!(outcome.decision, ApprovalDecision::Reject),
            "expected create failure to reject, got {:?}",
            outcome.decision
        );
        assert!(
            outcome
                .reason
                .as_deref()
                .is_some_and(|reason| reason.contains("review request failed")),
            "unexpected reason: {:?}",
            outcome.reason
        );
    }

    #[tokio::test]
    async fn test_review_manager_handler_wait_failure_rejects() {
        let manager = make_review_manager_with_store(Arc::new(FailingReviewStore {
            mode: FailingStoreMode::Get,
        }));
        let handler = ReviewManagerApprovalHandler::new(manager, "test-run-wait-fail")
            .with_review_timeout(std::time::Duration::from_millis(50));

        let outcome = handler
            .request_approval(ApprovalRequest {
                subtask_id: "task-wait-fail".into(),
                description: "Wait for review should fail".into(),
                risk_level: 0.4,
                prior_output: None,
            })
            .await;

        assert!(
            matches!(outcome.decision, ApprovalDecision::Reject),
            "expected wait failure to reject, got {:?}",
            outcome.decision
        );
        assert!(
            outcome
                .reason
                .as_deref()
                .is_some_and(|reason| reason.contains("review wait failed")),
            "unexpected reason: {:?}",
            outcome.reason
        );
    }

    #[tokio::test]
    async fn test_review_manager_handler_changes_requested_maps_to_modify() {
        let manager = make_review_manager();
        let mgr_clone = Arc::clone(&manager);
        let handler = ReviewManagerApprovalHandler::new(Arc::clone(&manager), "test-run-modify")
            .with_review_timeout(std::time::Duration::from_secs(5));

        let resolver = task::spawn(async move {
            loop {
                tokio::time::sleep(std::time::Duration::from_millis(50)).await;
                let pending = mgr_clone.list_pending(None, Some(1)).await.unwrap();
                if let Some(req) = pending.into_iter().next() {
                    mgr_clone
                        .resolve_review(
                            &req.id,
                            ReviewResponse::ChangesRequested {
                                changes: "Use a safer deployment plan".into(),
                                comment: None,
                            },
                            "auto-reviewer".into(),
                        )
                        .await
                        .unwrap();
                    return;
                }
            }
        });

        let outcome = handler
            .request_approval(ApprovalRequest {
                subtask_id: "task-modify".into(),
                description: "Deploy to production".into(),
                risk_level: 0.7,
                prior_output: None,
            })
            .await;
        resolver.await.unwrap();

        assert!(
            matches!(
                outcome.decision,
                ApprovalDecision::Modify(ref prompt) if prompt == "Use a safer deployment plan"
            ),
            "expected modify outcome, got {:?}",
            outcome.decision
        );
    }

    #[tokio::test]
    async fn test_review_manager_handler_deferred_auto_approves() {
        let manager = make_review_manager();
        let mgr_clone = Arc::clone(&manager);
        let handler = ReviewManagerApprovalHandler::new(Arc::clone(&manager), "test-run-deferred")
            .with_review_timeout(std::time::Duration::from_secs(5));

        let resolver = task::spawn(async move {
            loop {
                tokio::time::sleep(std::time::Duration::from_millis(50)).await;
                let pending = mgr_clone.list_pending(None, Some(1)).await.unwrap();
                if let Some(req) = pending.into_iter().next() {
                    mgr_clone
                        .resolve_review(
                            &req.id,
                            ReviewResponse::Deferred {
                                reason: "need another reviewer".into(),
                            },
                            "auto-reviewer".into(),
                        )
                        .await
                        .unwrap();
                    return;
                }
            }
        });

        let outcome = handler
            .request_approval(ApprovalRequest {
                subtask_id: "task-deferred".into(),
                description: "Review later".into(),
                risk_level: 0.3,
                prior_output: None,
            })
            .await;
        resolver.await.unwrap();

        assert!(
            matches!(outcome.decision, ApprovalDecision::Approve),
            "expected deferred review to auto-approve, got {:?}",
            outcome.decision
        );
    }
}
