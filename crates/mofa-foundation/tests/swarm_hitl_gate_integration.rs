//! Integration tests for `SwarmHITLGate`.
//!
//! These tests exercise the gate through its public API only, using the same
//! in-process `ReviewManager` stack that production code uses — no mocking,
//! no external dependencies.

use std::sync::Arc;
use std::time::Duration;

use mofa_foundation::hitl::{
    InMemoryReviewStore, ReviewManager, ReviewManagerConfig, ReviewNotifier, ReviewPolicyEngine,
};
use mofa_foundation::swarm::{
    FailurePolicy, HITLDecision, HITLGateMetrics, HITLMode, HITLNotifier, ParallelScheduler,
    RiskLevel, SchedulerSummary, SequentialScheduler, SubtaskDAG, SubtaskExecutorFn,
    SwarmHITLGate, SwarmScheduler, SwarmSchedulerConfig, SwarmSubtask, TaskOutcome,
};
use mofa_kernel::hitl::ReviewResponse;

// ── Helpers ───────────────────────────────────────────────────────────────────

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

fn echo_executor() -> SubtaskExecutorFn {
    Arc::new(|_idx, task: SwarmSubtask| {
        Box::pin(async move { Ok(format!("{}-done", task.id)) })
    })
}

/// Spawn a background task that polls `ReviewManager` for pending reviews and
/// resolves each one with the provided `response`.  Stops after the first
/// non-empty batch or when the deadline is exceeded.
fn spawn_auto_resolver(manager: Arc<ReviewManager>, response: ReviewResponse) {
    tokio::spawn(async move {
        let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
        loop {
            let pending = manager.list_pending(None, None).await.unwrap();
            if !pending.is_empty() {
                for r in pending {
                    manager
                        .resolve_review(&r.id, response.clone(), "auto-resolver".to_string())
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
}

// ── Test 1: HITLMode::None bypasses even Critical tasks ──────────────────────

#[tokio::test]
async fn test_gate_none_mode_bypasses_all_tasks() {
    let manager = make_manager();
    let gate = Arc::new(SwarmHITLGate::new(
        Arc::clone(&manager),
        HITLMode::None,
        "exec-1",
    ));
    let executor = gate.wrap_executor(echo_executor());

    let mut dag = SubtaskDAG::new("none-mode");
    dag.add_task(
        SwarmSubtask::new("critical-task", "Delete everything")
            .with_risk_level(RiskLevel::Critical),
    );

    let summary = SequentialScheduler::with_config(SwarmSchedulerConfig::default())
        .execute(&mut dag, executor)
        .await
        .unwrap();

    assert_eq!(summary.succeeded, 1);
    assert_eq!(summary.failed, 0);
    let pending = manager.list_pending(None, None).await.unwrap();
    assert!(pending.is_empty(), "HITLMode::None must never submit reviews");
}

// ── Test 2: HITLMode::Required intercepts even Low-risk tasks ────────────────

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
    dag.add_task(
        SwarmSubtask::new("low-task", "Search the web").with_risk_level(RiskLevel::Low),
    );

    spawn_auto_resolver(Arc::clone(&manager), ReviewResponse::Approved { comment: None });

    let summary = SequentialScheduler::with_config(SwarmSchedulerConfig::default())
        .execute(&mut dag, executor)
        .await
        .unwrap();

    assert_eq!(summary.succeeded, 1, "Low-risk task must succeed after approval");
}

// ── Test 3: HITLMode::Optional intercepts High-risk tasks ────────────────────

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

    spawn_auto_resolver(
        Arc::clone(&manager),
        ReviewResponse::Approved { comment: Some("LGTM".to_string()) },
    );

    let summary = SequentialScheduler::with_config(SwarmSchedulerConfig::default())
        .execute(&mut dag, executor)
        .await
        .unwrap();

    assert_eq!(summary.succeeded, 1);
}

// ── Test 4: HITLMode::Optional passes Low-risk tasks through directly ─────────

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
    dag.add_task(
        SwarmSubtask::new("low-task", "Search the web").with_risk_level(RiskLevel::Low),
    );

    let summary = SequentialScheduler::with_config(SwarmSchedulerConfig::default())
        .execute(&mut dag, executor)
        .await
        .unwrap();

    assert_eq!(summary.succeeded, 1);
    let pending = manager.list_pending(None, None).await.unwrap();
    assert!(pending.is_empty(), "Low-risk task in Optional mode must not be reviewed");
}

// ── Test 5: Rejected review causes task failure ───────────────────────────────

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

    spawn_auto_resolver(
        Arc::clone(&manager),
        ReviewResponse::Rejected {
            reason: "Not safe to deploy now".to_string(),
            comment: None,
        },
    );

    let mut cfg = SwarmSchedulerConfig::default();
    cfg.failure_policy = FailurePolicy::Continue;
    let summary = SequentialScheduler::with_config(cfg)
        .execute(&mut dag, executor)
        .await
        .unwrap();

    assert_eq!(summary.succeeded, 0);
    assert_eq!(summary.failed, 1, "Rejected task must be marked Failed");

    if let TaskOutcome::Failure(reason) = &summary.results[0].outcome {
        assert!(
            reason.contains("Not safe to deploy now"),
            "failure reason must include reviewer message: {reason}"
        );
    } else {
        panic!("Expected Failure outcome, got: {:?}", summary.results[0].outcome);
    }
}

// ── Test 6: ChangesRequested applies modified description before execution ────

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_gate_changes_requested_modifies_task_description() {
    let manager = make_manager();
    let gate = Arc::new(SwarmHITLGate::new(
        Arc::clone(&manager),
        HITLMode::Required,
        "exec-cr",
    ));

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

    spawn_auto_resolver(
        Arc::clone(&manager),
        ReviewResponse::ChangesRequested {
            changes: "Please add safety checks".to_string(),
            comment: None,
        },
    );

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
        "original task description must be preserved: {desc}"
    );
}

// ── Test 7: Optional mode auto-approves when review times out ─────────────────

#[tokio::test]
async fn test_gate_optional_mode_auto_approves_on_timeout() {
    let manager = make_manager();
    let gate = Arc::new(
        SwarmHITLGate::new(Arc::clone(&manager), HITLMode::Optional, "exec-timeout")
            .with_review_timeout(Duration::from_millis(50)),
    );
    let executor = gate.wrap_executor(echo_executor());

    let mut dag = SubtaskDAG::new("optional-timeout");
    dag.add_task(
        SwarmSubtask::new("timeout-task", "High risk but optional gate")
            .with_risk_level(RiskLevel::High),
    );

    // Nobody resolves the review — the gate times out.
    // In Optional mode this must auto-approve rather than fail the task.
    let summary = SequentialScheduler::with_config(SwarmSchedulerConfig::default())
        .execute(&mut dag, executor)
        .await
        .unwrap();

    assert_eq!(
        summary.succeeded, 1,
        "Optional gate timeout must auto-approve, not fail the task"
    );
}

// ── Test 8: Gate works with ParallelScheduler on a diamond DAG ───────────────

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
    let fetch =
        dag.add_task(SwarmSubtask::new("fetch", "Fetch data").with_risk_level(RiskLevel::Low));
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

    spawn_auto_resolver(Arc::clone(&manager), ReviewResponse::Approved { comment: None });

    let summary = ParallelScheduler::with_config(SwarmSchedulerConfig::default())
        .execute(&mut dag, executor)
        .await
        .unwrap();

    assert_eq!(summary.succeeded, 4, "all 4 tasks must succeed");
    assert_eq!(summary.failed, 0);
}

// ── Test 9: HITLGateMetrics counts intercepted/approved/rejected correctly ───

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_gate_metrics_track_decisions_correctly() {
    let manager = make_manager();
    let gate = Arc::new(SwarmHITLGate::new(
        Arc::clone(&manager),
        HITLMode::Optional,
        "exec-metrics",
    ));
    let executor = gate.clone().wrap_executor(echo_executor());

    // high-a and high-b get intercepted; low bypasses the gate
    let mut dag = SubtaskDAG::new("metrics-dag");
    dag.add_task(
        SwarmSubtask::new("high-a", "Write to database").with_risk_level(RiskLevel::High),
    );
    dag.add_task(
        SwarmSubtask::new("high-b", "Deploy service").with_risk_level(RiskLevel::Critical),
    );
    dag.add_task(SwarmSubtask::new("low", "Read config").with_risk_level(RiskLevel::Low));

    // Approve high-a, reject high-b
    let mgr = Arc::clone(&manager);
    tokio::spawn(async move {
        let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
        let mut approved_one = false;
        loop {
            let pending = mgr.list_pending(None, None).await.unwrap();
            for r in &pending {
                let response = if !approved_one {
                    approved_one = true;
                    ReviewResponse::Approved { comment: None }
                } else {
                    ReviewResponse::Rejected {
                        reason: "blocked".to_string(),
                        comment: None,
                    }
                };
                mgr.resolve_review(&r.id, response, "tester".to_string())
                    .await
                    .unwrap();
            }
            if approved_one && pending.len() >= 2 {
                break;
            }
            if tokio::time::Instant::now() >= deadline {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    });

    let mut cfg = SwarmSchedulerConfig::default();
    cfg.failure_policy = FailurePolicy::Continue;
    let summary = SequentialScheduler::with_config(cfg)
        .execute(&mut dag, executor)
        .await
        .unwrap();

    let metrics = gate.metrics();
    assert_eq!(metrics.intercepted, 2, "two high-risk tasks must be intercepted");

    // Every intercepted task must resolve to exactly one outcome.
    let resolved =
        metrics.approved + metrics.rejected + metrics.modified + metrics.auto_approved_timeout;
    assert_eq!(
        resolved, metrics.intercepted,
        "all intercepted tasks must have a resolved outcome: {:?}",
        metrics
    );

    // enrich_summary attaches metrics to the summary
    let summary = gate.enrich_summary(summary);
    assert!(
        summary.hitl_stats.is_some(),
        "enrich_summary must populate hitl_stats"
    );
    let attached = summary.hitl_stats.unwrap();
    assert_eq!(attached.intercepted, metrics.intercepted);
}

// ── Test 10: with_intercept_when overrides the built-in risk threshold ────────

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_gate_custom_predicate_overrides_risk_threshold() {
    let manager = make_manager();
    // Gate is in Required mode but we install a predicate that only intercepts
    // tasks with a specific capability requirement — not based on risk at all.
    let gate = Arc::new(
        SwarmHITLGate::new(Arc::clone(&manager), HITLMode::Required, "exec-pred")
            .with_intercept_when(|task| {
                task.required_capabilities
                    .contains(&"write_production_db".to_string())
            }),
    );
    let executor = gate.clone().wrap_executor(echo_executor());

    let mut dag = SubtaskDAG::new("predicate-dag");
    // This task has the capability — must be intercepted even though it's Low risk.
    let mut sensitive = SwarmSubtask::new("db-write", "Write to prod DB");
    sensitive.required_capabilities = vec!["write_production_db".to_string()];
    sensitive = sensitive.with_risk_level(RiskLevel::Low);
    dag.add_task(sensitive);

    // This task is Critical risk but lacks the capability — must NOT be intercepted.
    dag.add_task(
        SwarmSubtask::new("compute", "Heavy number crunching")
            .with_risk_level(RiskLevel::Critical),
    );

    spawn_auto_resolver(Arc::clone(&manager), ReviewResponse::Approved { comment: None });

    let summary = SequentialScheduler::with_config(SwarmSchedulerConfig::default())
        .execute(&mut dag, executor)
        .await
        .unwrap();

    assert_eq!(summary.succeeded, 2);

    let metrics = gate.metrics();
    // Only the db-write task (capability match) must have been intercepted.
    assert_eq!(
        metrics.intercepted, 1,
        "only the task matching the predicate must be intercepted: {:?}",
        metrics
    );
    assert_eq!(metrics.approved, 1);
}

// ── Test 11: HITLNotifier receives on_intercepted and on_decision calls ───────

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_gate_notifier_receives_events() {
    use std::sync::Mutex;

    struct RecordingNotifier {
        intercepted: Mutex<Vec<String>>,
        decisions: Mutex<Vec<(String, HITLDecision)>>,
    }

    impl HITLNotifier for RecordingNotifier {
        fn on_intercepted(&self, task: &SwarmSubtask) {
            self.intercepted.lock().unwrap().push(task.id.clone());
        }
        fn on_decision(&self, task: &SwarmSubtask, decision: HITLDecision, _latency_ms: u64) {
            self.decisions.lock().unwrap().push((task.id.clone(), decision));
        }
    }

    let notifier = Arc::new(RecordingNotifier {
        intercepted: Mutex::new(vec![]),
        decisions: Mutex::new(vec![]),
    });

    let manager = make_manager();
    let gate = Arc::new(
        SwarmHITLGate::new(Arc::clone(&manager), HITLMode::Required, "exec-notifier")
            .with_notifier(Arc::clone(&notifier) as Arc<dyn HITLNotifier>),
    );
    let executor = gate.clone().wrap_executor(echo_executor());

    let mut dag = SubtaskDAG::new("notifier-dag");
    dag.add_task(
        SwarmSubtask::new("task-a", "Do something safe").with_risk_level(RiskLevel::Low),
    );

    spawn_auto_resolver(Arc::clone(&manager), ReviewResponse::Approved { comment: None });

    let summary = SequentialScheduler::with_config(SwarmSchedulerConfig::default())
        .execute(&mut dag, executor)
        .await
        .unwrap();

    assert_eq!(summary.succeeded, 1);

    let intercepted = notifier.intercepted.lock().unwrap();
    let decisions = notifier.decisions.lock().unwrap();

    assert_eq!(intercepted.len(), 1, "notifier must receive one on_intercepted call");
    assert_eq!(intercepted[0], "task-a");
    assert_eq!(decisions.len(), 1, "notifier must receive one on_decision call");
    assert_eq!(decisions[0].0, "task-a");
    assert_eq!(decisions[0].1, HITLDecision::Approved);
}
