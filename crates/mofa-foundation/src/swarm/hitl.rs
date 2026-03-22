//! HITL (Human in the Loop) approval workflow for swarm schedulers

use async_trait::async_trait;
use std::sync::Arc;
use tokio::sync::{mpsc, oneshot, Mutex};
use tracing::Instrument;

use mofa_kernel::agent::types::error::GlobalResult;

use crate::swarm::config::{AuditEvent, AuditEventKind, HITLMode};
use crate::swarm::dag::RiskLevel;
use crate::swarm::scheduler::SubtaskExecutorFn;

fn approval_decision_label(decision: &ApprovalDecision) -> &'static str {
    match decision {
        ApprovalDecision::Approve => "approved",
        ApprovalDecision::Reject => "rejected",
        ApprovalDecision::Modify(_) => "modified",
    }
}

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
    pub risk_level: RiskLevel,
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
        // Span covers the channel round-trip; decision and channel_closed are recorded after the reviewer responds.
        let span = tracing::info_span!(
            "hitl.channel_approval",
            subtask_id = %req.subtask_id,
            risk_level = ?req.risk_level,
            decision = tracing::field::Empty,
            channel_closed = tracing::field::Empty,
        );
        let record_span = span.clone();

        let (reply_tx, reply_rx) = oneshot::channel();
        async move {
            // If the receiver is gone, default to Approve so the scheduler doesn't deadlock.
            if self.tx.send((req, reply_tx)).await.is_err() {
                record_span.record("channel_closed", true);
                record_span.record("decision", "approved");
                return ApprovalOutcome::approve();
            }

            let outcome = reply_rx.await.unwrap_or_else(|_| ApprovalOutcome::approve());
            record_span.record("channel_closed", false);
            record_span.record("decision", approval_decision_label(&outcome.decision));
            outcome
        }
        .instrument(span)
        .await
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
            let risk_level = task.risk_level.clone();

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
                        risk_level: risk_level.clone(),
                    };

                    audit.lock().await.push(AuditEvent::new(
                        AuditEventKind::HITLRequested,
                        format!("Approval requested for subtask '{id}'"),
                    ).with_data(serde_json::json!({ "subtask_id": id, "risk": format!("{:?}", risk_level) })));

                    // Span measures total wall-time waiting for human approval; decision is recorded after await.
                    let span = tracing::info_span!(
                        "hitl.approval_gate",
                        subtask_id = %id,
                        risk_level = ?risk_level,
                        hitl_required = task.hitl_required,
                        mode = "required",
                        decision = tracing::field::Empty,
                    );
                    let outcome = handler.request_approval(req).instrument(span.clone()).await;
                    span.record("decision", match &outcome.decision {
                        ApprovalDecision::Approve => "approved",
                        ApprovalDecision::Reject => "rejected",
                        ApprovalDecision::Modify(_) => "modified",
                    });

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
                    // Pre-filter: skip the gate entirely for low-risk tasks that don't have hitl_required set
                    if !task.hitl_required && !risk_level.requires_hitl() {
                        return base(idx, task).await;
                    }

                    let req = ApprovalRequest {
                        subtask_id: id.clone(),
                        description: desc.clone(),
                        prior_output: prior_output.clone(),
                        risk_level: risk_level.clone(),
                    };

                    audit.lock().await.push(AuditEvent::new(
                        AuditEventKind::HITLRequested,
                        format!("Optional approval requested for subtask '{id}'"),
                    ).with_data(serde_json::json!({ "subtask_id": id, "risk": format!("{:?}", risk_level) })));

                    // Span covers the optional timeout window; timed_out and decision are filled after the future settles.
                    let span = tracing::info_span!(
                        "hitl.approval_gate",
                        subtask_id = %id,
                        risk_level = ?risk_level,
                        hitl_required = task.hitl_required,
                        mode = "optional",
                        decision = tracing::field::Empty,
                        timed_out = tracing::field::Empty,
                    );
                    let outcome_result = tokio::time::timeout(
                        optional_timeout,
                        handler.request_approval(req).instrument(span.clone()),
                    )
                    .await;

                    let (outcome, timed_out) = match outcome_result {
                        Ok(outcome) => (outcome, false),
                        Err(_) => (ApprovalOutcome::approve(), true),
                    };
                    span.record("timed_out", timed_out);
                    span.record("decision", match &outcome.decision {
                        ApprovalDecision::Approve => if timed_out { "auto-approved" } else { "approved" },
                        ApprovalDecision::Reject => "rejected",
                        ApprovalDecision::Modify(_) => "modified",
                    });

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


#[cfg(test)]
mod tests {
    use super::*;
    use crate::swarm::dag::{SubtaskDAG, SwarmSubtask};
    use crate::swarm::scheduler::{SequentialScheduler, SwarmScheduler, SwarmSchedulerConfig};
    use std::collections::{BTreeMap, HashMap};
    use std::sync::{Mutex as StdMutex, OnceLock};
    use tokio::task;
    use tracing::field::{Field, Visit};
    use tracing_subscriber::layer::{Context, Layer, SubscriberExt};
    use tracing_subscriber::registry::LookupSpan;

    #[derive(Clone, Debug)]
    struct CapturedSpan {
        name: String,
        fields: BTreeMap<String, String>,
    }

    #[derive(Default)]
    struct TestSpanCollector {
        spans: StdMutex<HashMap<tracing::Id, CapturedSpan>>,
    }

    #[derive(Clone)]
    struct TestSpanLayer(std::sync::Arc<TestSpanCollector>);

    #[derive(Default)]
    struct FieldRecorder(BTreeMap<String, String>);

    impl Visit for FieldRecorder {
        fn record_bool(&mut self, field: &Field, value: bool) {
            self.0.insert(field.name().to_string(), value.to_string());
        }

        fn record_i64(&mut self, field: &Field, value: i64) {
            self.0.insert(field.name().to_string(), value.to_string());
        }

        fn record_u64(&mut self, field: &Field, value: u64) {
            self.0.insert(field.name().to_string(), value.to_string());
        }

        fn record_str(&mut self, field: &Field, value: &str) {
            self.0.insert(field.name().to_string(), value.to_string());
        }

        fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
            self.0.insert(field.name().to_string(), format!("{value:?}"));
        }
    }

    impl<S> Layer<S> for TestSpanLayer
    where
        S: tracing::Subscriber + for<'lookup> LookupSpan<'lookup>,
    {
        fn on_new_span(
            &self,
            attrs: &tracing::span::Attributes<'_>,
            id: &tracing::Id,
            _ctx: Context<'_, S>,
        ) {
            let mut recorder = FieldRecorder::default();
            attrs.record(&mut recorder);
            self.0.spans.lock().unwrap().insert(
                id.clone(),
                CapturedSpan {
                    name: attrs.metadata().name().to_string(),
                    fields: recorder.0,
                },
            );
        }

        fn on_record(
            &self,
            id: &tracing::Id,
            values: &tracing::span::Record<'_>,
            _ctx: Context<'_, S>,
        ) {
            if let Some(span) = self.0.spans.lock().unwrap().get_mut(id) {
                let mut recorder = FieldRecorder::default();
                values.record(&mut recorder);
                span.fields.extend(recorder.0);
            }
        }
    }

    impl TestSpanCollector {
        fn snapshot(&self) -> Vec<CapturedSpan> {
            self.spans.lock().unwrap().values().cloned().collect()
        }

        fn clear(&self) {
            self.spans.lock().unwrap().clear();
        }
    }

    static TEST_TRACING: OnceLock<(Arc<TestSpanCollector>, StdMutex<()>)> = OnceLock::new();

    fn test_tracing() -> &'static (Arc<TestSpanCollector>, StdMutex<()>) {
        TEST_TRACING.get_or_init(|| {
            let collector = Arc::new(TestSpanCollector::default());
            let subscriber = tracing_subscriber::registry().with(TestSpanLayer(collector.clone()));
            tracing::subscriber::set_global_default(subscriber)
                .expect("global test tracing subscriber should be set once");
            (collector, StdMutex::new(()))
        })
    }

    struct SlowApprovalHandler {
        delay: std::time::Duration,
    }

    #[async_trait]
    impl ApprovalHandler for SlowApprovalHandler {
        async fn request_approval(&self, _req: ApprovalRequest) -> ApprovalOutcome {
            tokio::time::sleep(self.delay).await;
            ApprovalOutcome::approve()
        }
    }

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

    #[tokio::test]
    async fn test_tracing_channel_approval_records_decision() {
        let (collector, guard_lock) = test_tracing();
        let _guard = guard_lock.lock().unwrap();
        collector.clear();
        let (handler, mut rx) = ChannelApprovalHandler::new(1);

        let reviewer = task::spawn(async move {
            let (_, reply) = rx.recv().await.expect("expected approval request");
            reply.send(ApprovalOutcome::approve()).ok();
        });

        let outcome = handler
            .request_approval(ApprovalRequest {
                subtask_id: "trace-task".into(),
                description: "Trace this approval".into(),
                prior_output: None,
                risk_level: RiskLevel::High,
            })
            .await;

        reviewer.await.unwrap();
        assert!(matches!(outcome.decision, ApprovalDecision::Approve));

        let spans = collector.snapshot();
        let span = spans
            .iter()
            .find(|span| span.name == "hitl.channel_approval")
            .unwrap_or_else(|| panic!("missing hitl.channel_approval span: {spans:#?}"));
        assert_eq!(span.fields.get("subtask_id").map(String::as_str), Some("trace-task"));
        assert_eq!(span.fields.get("decision").map(String::as_str), Some("approved"));
        assert_eq!(span.fields.get("channel_closed").map(String::as_str), Some("false"));
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
    #[tokio::test]
    async fn test_tracing_optional_gate_records_timeout() {
        let (collector, guard_lock) = test_tracing();
        let _guard = guard_lock.lock().unwrap();
        collector.clear();
        let mut dag = SubtaskDAG::new("test");
        let t1 = dag.add_task(SwarmSubtask::new("t1", "Run the analysis"));
        dag.get_task_mut(t1).unwrap().hitl_required = true;
        let audit_log = Arc::new(Mutex::new(vec![]));
        let executor = hitl_executor_middleware(
            mock_base_executor(),
            HITLMode::Optional,
            Arc::new(SlowApprovalHandler {
                delay: std::time::Duration::from_millis(50),
            }),
            audit_log,
            std::time::Duration::from_millis(5),
        );

        let summary = SequentialScheduler::new()
            .execute(&mut dag, executor)
            .await
            .expect("scheduler should succeed");

        assert_eq!(summary.succeeded, 1);

        let spans = collector.snapshot();
        let span = spans
            .iter()
            .find(|span| {
                span.name == "hitl.approval_gate"
                    && span.fields.get("mode").map(String::as_str) == Some("optional")
            })
            .unwrap_or_else(|| panic!("missing optional hitl.approval_gate span: {spans:#?}"));
        assert_eq!(span.fields.get("timed_out").map(String::as_str), Some("true"));
        assert_eq!(span.fields.get("decision").map(String::as_str), Some("auto-approved"));
        assert_eq!(span.fields.get("subtask_id").map(String::as_str), Some("t1"));
    }
}
