use mofa_foundation::swarm::admission_gate::{
    AdmissionDecision, AdmissionPolicy, ComplexityBudgetPolicy, MaxTaskCountPolicy,
    PolicyVerdict, RequiredCapabilityPolicy, RiskBudgetPolicy, SwarmAdmissionGate,
};
use mofa_foundation::swarm::{RiskLevel, SubtaskDAG, SwarmSubtask};

fn make_dag(tasks: &[(&str, RiskLevel, f64, Vec<&str>)]) -> SubtaskDAG {
    let mut dag = SubtaskDAG::new("test");
    for (id, risk, complexity, caps) in tasks {
        let task = SwarmSubtask::new(*id, *id)
            .with_risk_level(risk.clone())
            .with_complexity(*complexity)
            .with_capabilities(caps.iter().map(|s| s.to_string()).collect());
        dag.add_task(task);
    }
    dag
}

#[test]
fn test_empty_gate_allows_any_dag() {
    let gate = SwarmAdmissionGate::new();
    let dag = make_dag(&[("t1", RiskLevel::Low, 0.5, vec![])]);
    let report = gate.evaluate(&dag);
    assert!(report.is_allowed());
    assert!(matches!(report.decision, AdmissionDecision::Allowed));
}

#[test]
fn test_max_task_count_allows_at_limit() {
    let gate = SwarmAdmissionGate::new().with_policy(MaxTaskCountPolicy { limit: 3 });
    let dag = make_dag(&[
        ("a", RiskLevel::Low, 0.1, vec![]),
        ("b", RiskLevel::Low, 0.1, vec![]),
        ("c", RiskLevel::Low, 0.1, vec![]),
    ]);
    assert!(gate.evaluate(&dag).is_allowed());
}

#[test]
fn test_max_task_count_denies_over_limit() {
    let gate = SwarmAdmissionGate::new().with_policy(MaxTaskCountPolicy { limit: 2 });
    let dag = make_dag(&[
        ("a", RiskLevel::Low, 0.1, vec![]),
        ("b", RiskLevel::Low, 0.1, vec![]),
        ("c", RiskLevel::Low, 0.1, vec![]),
    ]);
    let report = gate.evaluate(&dag);
    assert!(report.decision.is_denied());
    if let AdmissionDecision::Denied(msgs) = &report.decision {
        assert!(msgs[0].contains("3 tasks"));
    }
}

#[test]
fn test_risk_budget_allows_within_limits() {
    let gate = SwarmAdmissionGate::new().with_policy(RiskBudgetPolicy {
        max_critical: 1,
        max_high: 2,
    });
    let dag = make_dag(&[
        ("a", RiskLevel::Critical, 0.5, vec![]),
        ("b", RiskLevel::High, 0.5, vec![]),
    ]);
    assert!(gate.evaluate(&dag).is_allowed());
}

#[test]
fn test_risk_budget_denies_too_many_critical() {
    let gate = SwarmAdmissionGate::new().with_policy(RiskBudgetPolicy {
        max_critical: 1,
        max_high: 5,
    });
    let dag = make_dag(&[
        ("a", RiskLevel::Critical, 0.5, vec![]),
        ("b", RiskLevel::Critical, 0.5, vec![]),
    ]);
    assert!(gate.evaluate(&dag).decision.is_denied());
}

#[test]
fn test_risk_budget_denies_too_many_high() {
    let gate = SwarmAdmissionGate::new().with_policy(RiskBudgetPolicy {
        max_critical: 0,
        max_high: 1,
    });
    let dag = make_dag(&[
        ("a", RiskLevel::High, 0.5, vec![]),
        ("b", RiskLevel::High, 0.5, vec![]),
    ]);
    assert!(gate.evaluate(&dag).decision.is_denied());
}

#[test]
fn test_required_capability_allows_known_caps() {
    let gate = SwarmAdmissionGate::new().with_policy(RequiredCapabilityPolicy::new(["search", "write"]));
    let dag = make_dag(&[("t", RiskLevel::Low, 0.5, vec!["search"])]);
    assert!(gate.evaluate(&dag).is_allowed());
}

#[test]
fn test_required_capability_denies_unknown_cap() {
    let gate = SwarmAdmissionGate::new().with_policy(RequiredCapabilityPolicy::new(["search"]));
    let dag = make_dag(&[("t", RiskLevel::Low, 0.5, vec!["deploy"])]);
    let report = gate.evaluate(&dag);
    assert!(report.decision.is_denied());
    assert!(!report.task_verdicts.is_empty());
    assert_eq!(report.task_verdicts[0].task_id, "t");
}

#[test]
fn test_complexity_budget_warns_over_budget() {
    let gate = SwarmAdmissionGate::new().with_policy(ComplexityBudgetPolicy { max_total: 1.0 });
    let dag = make_dag(&[
        ("a", RiskLevel::Low, 0.8, vec![]),
        ("b", RiskLevel::Low, 0.8, vec![]),
    ]);
    let report = gate.evaluate(&dag);
    // warn, not deny
    assert!(report.is_allowed());
    assert!(matches!(report.decision, AdmissionDecision::AllowedWithWarnings(_)));
}

#[test]
fn test_complexity_budget_allows_under_budget() {
    let gate = SwarmAdmissionGate::new().with_policy(ComplexityBudgetPolicy { max_total: 2.0 });
    let dag = make_dag(&[("a", RiskLevel::Low, 0.5, vec![])]);
    assert!(matches!(gate.evaluate(&dag).decision, AdmissionDecision::Allowed));
}

#[test]
fn test_deny_takes_precedence_over_warn() {
    let gate = SwarmAdmissionGate::new()
        .with_policy(ComplexityBudgetPolicy { max_total: 0.1 }) // warn
        .with_policy(MaxTaskCountPolicy { limit: 0 }); // deny
    let dag = make_dag(&[("t", RiskLevel::Low, 0.9, vec![])]);
    assert!(gate.evaluate(&dag).decision.is_denied());
}

#[test]
fn test_multiple_policies_all_allow() {
    let gate = SwarmAdmissionGate::new()
        .with_policy(MaxTaskCountPolicy { limit: 10 })
        .with_policy(RiskBudgetPolicy { max_critical: 2, max_high: 5 })
        .with_policy(RequiredCapabilityPolicy::new(["search", "write", "deploy"]))
        .with_policy(ComplexityBudgetPolicy { max_total: 10.0 });
    let dag = make_dag(&[
        ("a", RiskLevel::High, 0.3, vec!["search"]),
        ("b", RiskLevel::Critical, 0.5, vec!["deploy"]),
    ]);
    assert!(matches!(gate.evaluate(&dag).decision, AdmissionDecision::Allowed));
}

#[test]
fn test_report_task_verdicts_populated_for_per_task_denials() {
    let gate = SwarmAdmissionGate::new()
        .with_policy(RequiredCapabilityPolicy::new(["search"]));
    let dag = make_dag(&[
        ("good", RiskLevel::Low, 0.1, vec!["search"]),
        ("bad", RiskLevel::Low, 0.1, vec!["unknown"]),
    ]);
    let report = gate.evaluate(&dag);
    assert!(report.decision.is_denied());
    assert_eq!(report.task_verdicts.len(), 1);
    assert_eq!(report.task_verdicts[0].task_id, "bad");
    assert_eq!(report.task_verdicts[0].policy, "required_capability");
}

#[test]
fn test_metrics_track_evaluation_outcomes() {
    let gate = SwarmAdmissionGate::new()
        .with_policy(MaxTaskCountPolicy { limit: 1 })
        .with_policy(ComplexityBudgetPolicy { max_total: 0.1 });

    // allowed
    let dag_ok = make_dag(&[("a", RiskLevel::Low, 0.05, vec![])]);
    gate.evaluate(&dag_ok);

    // denied
    let dag_big = make_dag(&[
        ("a", RiskLevel::Low, 0.05, vec![]),
        ("b", RiskLevel::Low, 0.05, vec![]),
    ]);
    gate.evaluate(&dag_big);

    // warned (1 task, complexity over budget)
    let dag_complex = make_dag(&[("a", RiskLevel::Low, 0.9, vec![])]);
    gate.evaluate(&dag_complex);

    let m = gate.metrics();
    assert_eq!(m.evaluations, 3);
    assert_eq!(m.allowed, 1);
    assert_eq!(m.denied, 1);
    assert_eq!(m.warned, 1);
}

#[test]
fn test_custom_policy_can_be_plugged_in() {
    struct NoOpPolicy;
    impl AdmissionPolicy for NoOpPolicy {
        fn name(&self) -> &str { "noop" }
    }

    struct AlwaysDenyPolicy;
    impl AdmissionPolicy for AlwaysDenyPolicy {
        fn name(&self) -> &str { "always_deny" }
        fn evaluate_dag(&self, _dag: &SubtaskDAG) -> PolicyVerdict {
            PolicyVerdict::Deny("custom denial".into())
        }
    }

    let gate = SwarmAdmissionGate::new()
        .with_policy(NoOpPolicy)
        .with_policy(AlwaysDenyPolicy);

    let dag = SubtaskDAG::new("test");
    let report = gate.evaluate(&dag);
    assert!(report.decision.is_denied());
    if let AdmissionDecision::Denied(msgs) = &report.decision {
        assert!(msgs.iter().any(|m| m.contains("custom denial")));
    }
}
