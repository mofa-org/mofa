//! HITL (Human in the Loop) approval workflow for swarm schedulers

use async_trait::async_trait;
use tokio::sync::{mpsc, oneshot};

use mofa_kernel::agent::{AgentContext, core::MoFAAgent};
use mofa_kernel::agent::types::error::GlobalResult;

use crate::swarm::config::{AuditEvent, AuditEventKind, HITLMode};
use crate::swarm::dag::{SubtaskDAG, SubtaskStatus};

#[derive(Debug, Clone)]
pub struct SubtaskOutput {
    pub subtask_id: String,
    pub agent_id: String,
    pub output: String,
}

#[derive(Debug)]
pub struct ExecutionResult {
    pub dag_id: String,
    pub task_count: usize,
    pub completed: usize,
    pub failed: usize,
    pub outputs: Vec<SubtaskOutput>,
}

fn find_matching_agent<'a>(
    agents: &'a mut Vec<Box<dyn MoFAAgent>>,
    required: &[String],
) -> Option<&'a mut Box<dyn MoFAAgent>> {
    agents.iter_mut().find(|a| required.iter().all(|cap| a.capabilities().has_tag(cap)))
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

// Scheduler
// Execute a [`SubtaskDAG`] sequentially, pausing before each task for human
// approval when `hitl_mode` requires it.
pub async fn run_sequential_with_hitl(
    dag: &mut SubtaskDAG,
    agents: &mut Vec<Box<dyn MoFAAgent>>,
    ctx: &AgentContext,
    hitl_mode: HITLMode,
    handler: &dyn ApprovalHandler,
    audit: &mut Vec<AuditEvent>,
) -> GlobalResult<ExecutionResult> {
    use mofa_kernel::agent::types::{AgentInput, error::GlobalError};

    let dag_id = dag.id.clone();
    let task_count = dag.task_count();
    let mut outputs: Vec<SubtaskOutput> = Vec::new();
    let mut last_output: Option<String> = None;

    while !dag.is_complete() {
        let ready = dag.ready_tasks();
        if ready.is_empty() { break; }

        let idx = ready[0];
        let (id, desc, caps, complexity) = {
            let t = dag.get_task(idx).unwrap();
            (t.id.clone(), t.description.clone(), t.required_capabilities.clone(), t.complexity)
        };

        // HITL gate
        let effective_desc = match hitl_mode {
            HITLMode::None => desc.clone(),
            HITLMode::Required => {
                let req = ApprovalRequest {
                    subtask_id: id.clone(),
                    description: desc.clone(),
                    prior_output: last_output.clone(),
                    risk_level: complexity,
                };

                audit.push(AuditEvent::new(
                    AuditEventKind::HITLRequested,
                    format!("Approval requested for subtask '{id}'"),
                ).with_data(serde_json::json!({ "subtask_id": id, "risk": complexity })));

                let outcome = handler.request_approval(req).await;

                let (decision_label, modified_desc) = match &outcome.decision {
                    ApprovalDecision::Approve => ("approved".to_string(), desc.clone()),
                    ApprovalDecision::Modify(p) => ("modified".to_string(), p.clone()),
                    ApprovalDecision::Reject => {
                        audit.push(AuditEvent::new(
                            AuditEventKind::HITLDecision,
                            format!("Subtask '{id}' rejected"),
                        ).with_data(serde_json::json!({
                            "subtask_id": id,
                            "decision": "reject",
                            "reason": outcome.reason,
                        })));
                        dag.mark_failed(idx, "rejected by reviewer");
                        return Err(GlobalError::Other(format!("Subtask '{id}' rejected by reviewer")));
                    }
                };

                let mut data = serde_json::json!({ "subtask_id": id, "decision": decision_label });
                if let ApprovalDecision::Modify(p) = &outcome.decision {
                    data["modified_prompt"] = serde_json::Value::String(p.clone());
                }
                if let Some(r) = &outcome.reason {
                    data["reason"] = serde_json::Value::String(r.clone());
                }
                audit.push(AuditEvent::new(
                    AuditEventKind::HITLDecision,
                    format!("Subtask '{id}' {decision_label}"),
                ).with_data(data));

                modified_desc
            }
            HITLMode::Optional => {
                let req = ApprovalRequest {
                    subtask_id: id.clone(),
                    description: desc.clone(),
                    prior_output: last_output.clone(),
                    risk_level: complexity,
                };

                audit.push(AuditEvent::new(
                    AuditEventKind::HITLRequested,
                    format!("Optional approval requested for subtask '{id}'"),
                ).with_data(serde_json::json!({ "subtask_id": id, "risk": complexity })));

                let outcome = tokio::time::timeout(
                    std::time::Duration::from_secs(5),
                    handler.request_approval(req),
                ).await.unwrap_or_else(|_| ApprovalOutcome::approve());

                let (decision_label, modified_desc) = match &outcome.decision {
                    ApprovalDecision::Approve => ("auto-approved".to_string(), desc.clone()),
                    ApprovalDecision::Modify(p) => ("modified".to_string(), p.clone()),
                    ApprovalDecision::Reject => {
                        audit.push(AuditEvent::new(
                            AuditEventKind::HITLDecision,
                            format!("Subtask '{id}' rejected (optional gate)"),
                        ).with_data(serde_json::json!({
                            "subtask_id": id, "decision": "reject", "reason": outcome.reason,
                        })));
                        dag.mark_failed(idx, "rejected by reviewer");
                        return Err(GlobalError::Other(format!("Subtask '{id}' rejected by reviewer")));
                    }
                };

                audit.push(AuditEvent::new(
                    AuditEventKind::HITLDecision,
                    format!("Subtask '{id}' {decision_label}"),
                ).with_data(serde_json::json!({ "subtask_id": id, "decision": decision_label })));

                modified_desc
            }
        };
        // End gate

        let agent = find_matching_agent(agents, &caps).ok_or_else(|| {
            GlobalError::Other(format!("No agent satisfies capabilities {:?} for subtask '{id}'", caps))
        })?;

        let agent_id = agent.id().to_string();
        dag.mark_running(idx);

        let input = AgentInput::text(format!("[{id}] {effective_desc}"));
        match agent.execute(input, ctx).await {
            Ok(out) => {
                let text = out.to_text();
                last_output = Some(text.clone());
                dag.mark_complete_with_output(idx, Some(text.clone()));
                outputs.push(SubtaskOutput { subtask_id: id, agent_id, output: text });
            }
            Err(e) => {
                let reason = e.to_string();
                dag.mark_failed(idx, &reason);
                return Err(GlobalError::Other(format!("Subtask '{id}' failed: {reason}")));
            }
        }
    }

    let completed = dag.all_tasks().iter().filter(|(_, t)| t.status == SubtaskStatus::Completed).count();
    let failed = dag.all_tasks().iter().filter(|(_, t)| matches!(t.status, SubtaskStatus::Failed(_))).count();

    Ok(ExecutionResult { dag_id, task_count, completed, failed, outputs })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::swarm::dag::{SubtaskDAG, SubtaskStatus, SwarmSubtask};
    use mofa_kernel::agent::{
        AgentCapabilities,
        error::AgentResult,
        types::{AgentOutput, AgentState, InterruptResult},
    };
    use tokio::task;

    fn ctx() -> AgentContext { AgentContext::new("hitl-test") }

    // An agent that echoes its input text
    struct EchoAgent { id: String, caps: AgentCapabilities }
    impl EchoAgent {
        fn new(id: impl Into<String>) -> Self {
            Self { id: id.into(), caps: AgentCapabilities::default() }
        }
    }
    #[async_trait::async_trait]
    impl MoFAAgent for EchoAgent {
        fn id(&self) -> &str { &self.id }
        fn name(&self) -> &str { &self.id }
        fn capabilities(&self) -> &AgentCapabilities { &self.caps }
        fn state(&self) -> AgentState { AgentState::Ready }
        async fn initialize(&mut self, _: &AgentContext) -> AgentResult<()> { Ok(()) }
        async fn execute(
            &mut self,
            input: mofa_kernel::agent::types::AgentInput,
            _: &AgentContext,
        ) -> AgentResult<AgentOutput> {
            Ok(AgentOutput::text(input.as_text().unwrap_or_default().to_string()))
        }
        async fn shutdown(&mut self) -> AgentResult<()> { Ok(()) }
        async fn interrupt(&mut self) -> AgentResult<InterruptResult> {
            Ok(InterruptResult::Acknowledged)
        }
    }

    fn one_task_dag() -> SubtaskDAG {
        let mut d = SubtaskDAG::new("test");
        d.add_task(SwarmSubtask::new("t1", "Run the analysis"));
        d
    }

    fn agents() -> Vec<Box<dyn MoFAAgent>> {
        vec![Box::new(EchoAgent::new("a1"))]
    }

    // use ChannelApprovalHandler
    /// Reviewer receives the request and sends Approve, task completes.
    #[tokio::test]
    async fn test_hitl_channel_approve() {
        let (handler, mut rx) = ChannelApprovalHandler::new(4);

        let reviewer = task::spawn(async move {
            let (req, reply) = rx.recv().await.expect("expected approval request");
            assert_eq!(req.subtask_id, "t1");
            assert!(!req.description.is_empty());
            reply.send(ApprovalOutcome::approve()).ok();
        });

        let mut dag = one_task_dag();
        let result = run_sequential_with_hitl(
            &mut dag, &mut agents(), &ctx(),
            HITLMode::Required, &handler, &mut vec![],
        ).await.unwrap();

        reviewer.await.unwrap();
        assert_eq!(result.completed, 1);
        assert_eq!(result.failed, 0);
    }

    /// Reviewer sends Reject, scheduler halts, downstream task stays Pending.
    #[tokio::test]
    async fn test_hitl_channel_reject() {
        let (handler, mut rx) = ChannelApprovalHandler::new(4);

        let mut dag = SubtaskDAG::new("chain");
        let a = dag.add_task(SwarmSubtask::new("a", "Step 1"));
        let b = dag.add_task(SwarmSubtask::new("b", "Step 2"));
        dag.add_dependency(a, b).unwrap();

        let reviewer = task::spawn(async move {
            let (req, reply) = rx.recv().await.expect("expected approval request");
            assert_eq!(req.subtask_id, "a");
            reply.send(ApprovalOutcome::reject("too risky")).ok();
        });

        let result = run_sequential_with_hitl(
            &mut dag, &mut agents(), &ctx(),
            HITLMode::Required, &handler, &mut vec![],
        ).await;

        reviewer.await.unwrap();
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("rejected"));
        assert_eq!(dag.get_task(b).unwrap().status, SubtaskStatus::Pending);
    }

    /// Reviewer sends Modify, task runs with the reviewer's revised prompt.
    #[tokio::test]
    async fn test_hitl_channel_modify_prompt() {
        let (handler, mut rx) = ChannelApprovalHandler::new(4);

        let reviewer = task::spawn(async move {
            let (req, reply) = rx.recv().await.expect("expected approval request");
            assert!(req.description.contains("Run the analysis"));
            reply.send(ApprovalOutcome::modify("Summarise only the key findings")).ok();
        });

        let mut dag = one_task_dag();
        let result = run_sequential_with_hitl(
            &mut dag, &mut agents(), &ctx(),
            HITLMode::Required, &handler, &mut vec![],
        ).await.unwrap();

        reviewer.await.unwrap();
        assert_eq!(result.completed, 1);
        assert!(result.outputs[0].output.contains("Summarise only the key findings"),
            "output was: {}", result.outputs[0].output);
    }

    #[tokio::test]
    async fn test_hitl_none_skips_channel() {
        let (handler, rx) = ChannelApprovalHandler::new(4);
        drop(rx);

        let mut dag = one_task_dag();
        let result = run_sequential_with_hitl(
            &mut dag, &mut agents(), &ctx(),
            HITLMode::None, &handler, &mut vec![],
        ).await.unwrap();

        assert_eq!(result.completed, 1);
    }

    /// audit log must contain HITLRequested + HITLDecision
    #[tokio::test]
    async fn test_hitl_audit_events_recorded() {
        let (handler, mut rx) = ChannelApprovalHandler::new(4);

        let reviewer = task::spawn(async move {
            let (_, reply) = rx.recv().await.expect("expected approval request");
            reply.send(ApprovalOutcome::approve()).ok();
        });

        let mut dag = one_task_dag();
        let mut audit: Vec<AuditEvent> = vec![];

        run_sequential_with_hitl(
            &mut dag, &mut agents(), &ctx(),
            HITLMode::Required, &handler, &mut audit,
        ).await.unwrap();

        reviewer.await.unwrap();
        let kinds: Vec<_> = audit.iter().map(|e| &e.kind).collect();
        assert!(kinds.contains(&&AuditEventKind::HITLRequested), "missing HITLRequested");
        assert!(kinds.contains(&&AuditEventKind::HITLDecision),  "missing HITLDecision");
    }
}
