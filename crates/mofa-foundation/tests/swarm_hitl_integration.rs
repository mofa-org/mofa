//! swarm HITL integration tests
//!
//! End-to-end tests that wire `hitl_executor_middleware` into real
//! `SequentialScheduler` / `ParallelScheduler` runs against a complete DAG.

use std::sync::Arc;
use tokio::sync::Mutex;
use futures::future::BoxFuture;
use petgraph::graph::NodeIndex;

use mofa_foundation::swarm::{
    AuditEventKind, FailurePolicy, HITLMode, SequentialScheduler, SubtaskDAG,
    SubtaskExecutorFn, SubtaskStatus, SwarmScheduler, SwarmSchedulerConfig,
    SwarmSubtask, hitl_executor_middleware,
};
use mofa_foundation::swarm::hitl::{
    ApprovalOutcome, ChannelApprovalHandler,
};

fn ok_executor() -> SubtaskExecutorFn {
    Arc::new(|_idx: NodeIndex, task: SwarmSubtask| -> BoxFuture<'static, _> {
        Box::pin(async move { Ok(format!("done:{}", task.description)) })
    })
}

// 1. Approve all nodes — full DAG completes successfully
#[tokio::test]
async fn hitl_approve_all_nodes_completes_dag() {
    let (handler, mut rx) = ChannelApprovalHandler::new(16);
    let handler_arc = Arc::new(handler);

    // 3-node chain: fetch → analyze → report
    let mut dag = SubtaskDAG::new("pipeline");
    let fetch_id = dag.add_task(SwarmSubtask::new("fetch", "Fetch raw data"));
    let analyze_id = dag.add_task(SwarmSubtask::new("analyze", "Analyze the data"));
    let report_id = dag.add_task(SwarmSubtask::new("report", "Generate report"));
    dag.add_dependency(fetch_id, analyze_id).unwrap();
    dag.add_dependency(analyze_id, report_id).unwrap();

    // Auto-approve all requests
    tokio::spawn(async move {
        while let Some((_req, reply)) = rx.recv().await {
            reply.send(ApprovalOutcome::approve()).ok();
        }
    });

    let audit_log = Arc::new(Mutex::new(vec![]));
    let config = SwarmSchedulerConfig::default();
    let executor = hitl_executor_middleware(
        ok_executor(), HITLMode::Required, handler_arc, audit_log.clone(), config.hitl_optional_timeout,
    );

    let scheduler = SequentialScheduler::new();
    let summary = scheduler.execute(&mut dag, executor).await.unwrap();

    assert_eq!(summary.succeeded, 3);
    assert_eq!(summary.failed, 0);
    assert_eq!(dag.get_task(report_id).unwrap().status, SubtaskStatus::Completed);

    // Audit log should have 3×2 = 6 entries (HITLRequested + HITLDecision per task)
    let audit = audit_log.lock().await;
    let requested = audit.iter().filter(|e| e.kind == AuditEventKind::HITLRequested).count();
    let decided = audit.iter().filter(|e| e.kind == AuditEventKind::HITLDecision).count();
    assert_eq!(requested, 3);
    assert_eq!(decided, 3);
}

// 2. Reject the first node — downstream stays Skipped (FailFastCascade)
#[tokio::test]
async fn hitl_reject_first_node_cascades_skip() {
    let (handler, mut rx) = ChannelApprovalHandler::new(4);
    let handler_arc = Arc::new(handler);

    let mut dag = SubtaskDAG::new("chain");
    let a = dag.add_task(SwarmSubtask::new("a", "Step A"));
    let b = dag.add_task(SwarmSubtask::new("b", "Step B"));
    let c = dag.add_task(SwarmSubtask::new("c", "Step C"));
    dag.add_dependency(a, b).unwrap();
    dag.add_dependency(b, c).unwrap();

    // Reject only the first task
    tokio::spawn(async move {
        if let Some((_req, reply)) = rx.recv().await {
            reply.send(ApprovalOutcome::reject("blocked")).ok();
        }
    });

    let audit_log = Arc::new(Mutex::new(vec![]));
    let config = SwarmSchedulerConfig::default();
    let executor = hitl_executor_middleware(
        ok_executor(), HITLMode::Required, handler_arc, audit_log, config.hitl_optional_timeout,
    );

    let config = SwarmSchedulerConfig {
        failure_policy: FailurePolicy::FailFastCascade,
        ..Default::default()
    };
    let scheduler = SequentialScheduler::with_config(config);
    let summary = scheduler.execute(&mut dag, executor).await.unwrap();

    assert_eq!(summary.failed, 1);
    assert_eq!(dag.get_task(b).unwrap().status, SubtaskStatus::Skipped);
    assert_eq!(dag.get_task(c).unwrap().status, SubtaskStatus::Skipped);
}

// 3. Modify the description — base executor receives the reviewer's prompt
#[tokio::test]
async fn hitl_modify_prompt_reaches_base_executor() {
    let (handler, mut rx) = ChannelApprovalHandler::new(4);
    let handler_arc = Arc::new(handler);

    let mut dag = SubtaskDAG::new("modify");
    dag.add_task(SwarmSubtask::new("t1", "Original description"));

    tokio::spawn(async move {
        if let Some((_req, reply)) = rx.recv().await {
            reply.send(ApprovalOutcome::modify("Reviewer changed this prompt")).ok();
        }
    });

    let received_desc = Arc::new(Mutex::new(String::new()));
    let received_desc_clone = received_desc.clone();

    let capturing_executor: SubtaskExecutorFn = Arc::new(move |_idx, task| {
        let desc_store = received_desc_clone.clone();
        Box::pin(async move {
            *desc_store.lock().await = task.description.clone();
            Ok(format!("captured: {}", task.description))
        })
    });

    let audit_log = Arc::new(Mutex::new(vec![]));
    let config = SwarmSchedulerConfig::default();
    let executor = hitl_executor_middleware(
        capturing_executor, HITLMode::Required, handler_arc, audit_log, config.hitl_optional_timeout,
    );

    let scheduler = SequentialScheduler::new();
    let summary = scheduler.execute(&mut dag, executor).await.unwrap();

    assert_eq!(summary.succeeded, 1);
    assert_eq!(*received_desc.lock().await, "Reviewer changed this prompt");
}

// 4. HITLMode::None — base executor called directly, no approval channel touched
#[tokio::test]
async fn hitl_none_mode_bypasses_approval() {
    let (handler, rx) = ChannelApprovalHandler::new(4);
    drop(rx); // Dropping rx ensures any send would fail

    let mut dag = SubtaskDAG::new("bypass");
    let a = dag.add_task(SwarmSubtask::new("a", "Task A"));
    let b = dag.add_task(SwarmSubtask::new("b", "Task B"));
    dag.add_dependency(a, b).unwrap();

    let audit_log = Arc::new(Mutex::new(vec![]));
    let config = SwarmSchedulerConfig::default();
    let executor = hitl_executor_middleware(
        ok_executor(), HITLMode::None, Arc::new(handler), audit_log.clone(), config.hitl_optional_timeout,
    );

    let scheduler = SequentialScheduler::new();
    let summary = scheduler.execute(&mut dag, executor).await.unwrap();

    assert_eq!(summary.succeeded, 2);
    // No audit events should have been recorded in None mode
    let audit = audit_log.lock().await;
    assert!(audit.is_empty());
}
