use mofa_foundation::swarm::{
    AgentSpec, SubtaskDAG, SwarmCapabilityRegistry, SwarmSubtask,
};

fn agent(id: &str, caps: &[&str]) -> AgentSpec {
    AgentSpec {
        id: id.into(),
        capabilities: caps.iter().map(|s| s.to_string()).collect(),
        model: None,
        cost_per_token: None,
        max_concurrency: 4,
    }
}

fn task_with_caps(id: &str, caps: &[&str]) -> SwarmSubtask {
    SwarmSubtask::new(id, id).with_capabilities(caps.iter().map(|s| s.to_string()).collect())
}

#[test]
fn test_empty_registry_agent_count() {
    let r = SwarmCapabilityRegistry::new();
    assert_eq!(r.agent_count(), 0);
    assert_eq!(r.capability_count(), 0);
}

#[test]
fn test_register_single_agent() {
    let r = SwarmCapabilityRegistry::new().register(agent("a1", &["summarize"]));
    assert_eq!(r.agent_count(), 1);
    assert_eq!(r.capability_count(), 1);
}

#[test]
fn test_register_multiple_agents() {
    let r = SwarmCapabilityRegistry::new()
        .register(agent("a1", &["summarize"]))
        .register(agent("a2", &["translate"]))
        .register(agent("a3", &["summarize", "translate"]));
    assert_eq!(r.agent_count(), 3);
    assert_eq!(r.capability_count(), 2);
}

#[test]
fn test_find_by_capability_returns_matching() {
    let r = SwarmCapabilityRegistry::new()
        .register(agent("a1", &["summarize"]))
        .register(agent("a2", &["translate"]));
    let found = r.find_by_capability("summarize");
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].id, "a1");
}

#[test]
fn test_find_by_capability_returns_empty_for_unknown() {
    let r = SwarmCapabilityRegistry::new().register(agent("a1", &["summarize"]));
    assert!(r.find_by_capability("code-review").is_empty());
}

#[test]
fn test_find_for_task_single_cap() {
    let r = SwarmCapabilityRegistry::new()
        .register(agent("a1", &["summarize"]))
        .register(agent("a2", &["translate"]));
    let task = task_with_caps("t1", &["summarize"]);
    let found = r.find_for_task(&task);
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].id, "a1");
}

#[test]
fn test_find_for_task_multi_cap_all_required() {
    let r = SwarmCapabilityRegistry::new()
        .register(agent("a1", &["summarize"]))
        .register(agent("a2", &["summarize", "translate"]))
        .register(agent("a3", &["translate"]));

    // only a2 has both capabilities
    let task = task_with_caps("t1", &["summarize", "translate"]);
    let found = r.find_for_task(&task);
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].id, "a2");
}

#[test]
fn test_find_for_task_no_caps_returns_all() {
    let r = SwarmCapabilityRegistry::new()
        .register(agent("a1", &["summarize"]))
        .register(agent("a2", &["translate"]));
    let task = SwarmSubtask::new("t1", "unconstrained task");
    assert_eq!(r.find_for_task(&task).len(), 2);
}

#[test]
fn test_agent_with_partial_caps_excluded() {
    let r = SwarmCapabilityRegistry::new()
        .register(agent("a1", &["summarize"])) // missing "translate"
        .register(agent("a2", &["summarize", "translate"]));

    let task = task_with_caps("t1", &["summarize", "translate"]);
    let found = r.find_for_task(&task);
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].id, "a2");
}

#[test]
fn test_coverage_report_all_covered() {
    let r = SwarmCapabilityRegistry::new()
        .register(agent("a1", &["summarize"]))
        .register(agent("a2", &["summarize"]));
    let mut dag = SubtaskDAG::new("dag");
    dag.add_task(task_with_caps("t1", &["summarize"]));

    let report = r.coverage_report(&dag);
    assert!(report.is_fully_covered());
    assert!(report.uncovered.is_empty());
    assert!(report.partial.is_empty());
}

#[test]
fn test_coverage_report_uncovered_task() {
    let r = SwarmCapabilityRegistry::new().register(agent("a1", &["summarize"]));
    let mut dag = SubtaskDAG::new("dag");
    dag.add_task(task_with_caps("t1", &["code-review"]));

    let report = r.coverage_report(&dag);
    assert!(!report.is_fully_covered());
    assert_eq!(report.uncovered, vec!["t1"]);
    assert!(report.gaps.contains(&"code-review".to_string()));
}

#[test]
fn test_coverage_report_partial_single_agent() {
    let r = SwarmCapabilityRegistry::new().register(agent("a1", &["summarize"]));
    let mut dag = SubtaskDAG::new("dag");
    dag.add_task(task_with_caps("t1", &["summarize"]));

    let report = r.coverage_report(&dag);
    assert!(report.is_fully_covered()); // uncovered is empty
    assert!(report.has_spof_risk());     // only 1 agent -> SPOF
    assert_eq!(report.partial, vec!["t1"]);
}

#[test]
fn test_coverage_report_mixed() {
    let r = SwarmCapabilityRegistry::new()
        .register(agent("a1", &["summarize"]))
        .register(agent("a2", &["summarize"]))
        .register(agent("a3", &["translate"]));

    let mut dag = SubtaskDAG::new("dag");
    dag.add_task(task_with_caps("covered", &["summarize"]));
    dag.add_task(task_with_caps("partial", &["translate"]));
    dag.add_task(task_with_caps("uncovered", &["code-review"]));

    let report = r.coverage_report(&dag);
    assert!(!report.is_fully_covered());
    assert_eq!(report.covered, vec!["covered"]);
    assert_eq!(report.partial, vec!["partial"]);
    assert_eq!(report.uncovered, vec!["uncovered"]);
    assert!(report.gaps.contains(&"code-review".to_string()));
    assert_eq!(report.problem_count(), 2);
}

#[test]
fn test_coverage_report_no_cap_task_always_covered() {
    let r = SwarmCapabilityRegistry::new(); // empty registry
    let mut dag = SubtaskDAG::new("dag");
    dag.add_task(SwarmSubtask::new("t1", "no cap required"));

    let report = r.coverage_report(&dag);
    assert!(report.is_fully_covered());
    assert!(report.covered.contains(&"t1".to_string()));
}

#[test]
fn test_agents_slice_matches_registered() {
    let r = SwarmCapabilityRegistry::new()
        .register(agent("a1", &["summarize"]))
        .register(agent("a2", &["translate"]));
    assert_eq!(r.agents().len(), 2);
    assert_eq!(r.agents()[0].id, "a1");
    assert_eq!(r.agents()[1].id, "a2");
}
