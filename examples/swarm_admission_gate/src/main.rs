use mofa_foundation::swarm::admission_gate::{
    AdmissionDecision, ComplexityBudgetPolicy, MaxTaskCountPolicy, RequiredCapabilityPolicy,
    RiskBudgetPolicy, SwarmAdmissionGate,
};
use mofa_foundation::swarm::{RiskLevel, SubtaskDAG, SwarmSubtask};

fn build_gate() -> SwarmAdmissionGate {
    SwarmAdmissionGate::new()
        .with_policy(MaxTaskCountPolicy { limit: 10 })
        .with_policy(RiskBudgetPolicy {
            max_critical: 1,
            max_high: 3,
        })
        .with_policy(RequiredCapabilityPolicy::new([
            "search", "summarise", "write", "deploy",
        ]))
        .with_policy(ComplexityBudgetPolicy { max_total: 4.0 })
}

fn print_report(label: &str, gate: &SwarmAdmissionGate, dag: &SubtaskDAG) {
    let report = gate.evaluate(dag);
    println!("\n── {label} ──");
    match &report.decision {
        AdmissionDecision::Allowed => println!("  decision : ALLOWED"),
        AdmissionDecision::AllowedWithWarnings(ws) => {
            println!("  decision : ALLOWED WITH WARNINGS");
            for w in ws {
                println!("    warn: {w}");
            }
        }
        AdmissionDecision::Denied(ds) => {
            println!("  decision : DENIED");
            for d in ds {
                println!("    deny: {d}");
            }
        }
        _ => {}
    }
    if !report.task_verdicts.is_empty() {
        println!("  task verdicts:");
        for tv in &report.task_verdicts {
            println!("    [{}] via policy '{}'", tv.task_id, tv.policy);
        }
    }
}

fn main() {
    tracing_subscriber::fmt().with_target(false).init();
    let gate = build_gate();

    // scenario 1: clean dag — should pass
    let mut dag1 = SubtaskDAG::new("research-pipeline");
    dag1.add_task(
        SwarmSubtask::new("fetch", "fetch source documents")
            .with_capabilities(vec!["search".into()])
            .with_risk_level(RiskLevel::Low)
            .with_complexity(0.3),
    );
    dag1.add_task(
        SwarmSubtask::new("summarise", "summarise findings")
            .with_capabilities(vec!["summarise".into()])
            .with_risk_level(RiskLevel::Low)
            .with_complexity(0.4),
    );
    dag1.add_task(
        SwarmSubtask::new("report", "write final report")
            .with_capabilities(vec!["write".into()])
            .with_risk_level(RiskLevel::Medium)
            .with_complexity(0.5),
    );
    print_report("clean pipeline", &gate, &dag1);

    // scenario 2: unknown capability — should deny
    let mut dag2 = SubtaskDAG::new("risky-pipeline");
    dag2.add_task(
        SwarmSubtask::new("fetch", "fetch data")
            .with_capabilities(vec!["search".into()])
            .with_complexity(0.3),
    );
    dag2.add_task(
        SwarmSubtask::new("exploit", "run exploit scanner")
            .with_capabilities(vec!["exploit".into()]) // not in allowed set
            .with_complexity(0.9),
    );
    print_report("unknown capability", &gate, &dag2);

    // scenario 3: too many critical tasks — should deny
    let mut dag3 = SubtaskDAG::new("critical-heavy");
    for i in 0..3 {
        dag3.add_task(
            SwarmSubtask::new(format!("deploy-{i}"), format!("deploy step {i}"))
                .with_capabilities(vec!["deploy".into()])
                .with_risk_level(RiskLevel::Critical)
                .with_complexity(0.5),
        );
    }
    print_report("too many critical tasks", &gate, &dag3);

    // scenario 4: complexity over budget — should warn but allow
    let mut dag4 = SubtaskDAG::new("complex-pipeline");
    for i in 0..5 {
        dag4.add_task(
            SwarmSubtask::new(format!("step-{i}"), format!("step {i}"))
                .with_capabilities(vec!["search".into()])
                .with_risk_level(RiskLevel::Low)
                .with_complexity(0.9),
        );
    }
    print_report("over complexity budget (warn only)", &gate, &dag4);

    println!();
}
