//! Demonstrates [`PatternSelector`] choosing the right coordination pattern
//! automatically from DAG topology — no LLM call, no API key required.
//!
//! Each section builds a DAG that matches one canonical pattern shape and
//! shows the selector's choice, confidence, and reason.
//!
//! Run: RUST_LOG=info cargo run -p swarm_pattern_selector

use anyhow::Result;
use mofa_foundation::swarm::{PatternSelector, RiskLevel, SubtaskDAG, SwarmSubtask};

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    println!("=== PatternSelector: auto-select coordination pattern from DAG topology ===\n");

    demo_routing();
    demo_supervision();
    demo_debate();
    demo_consensus();
    demo_mapreduce();
    demo_sequential();
    demo_parallel();

    println!("\n=== Done — zero LLM calls, zero API keys ===");
    Ok(())
}

fn print_result(label: &str, dag: &SubtaskDAG) {
    let sel = PatternSelector::select_with_reason(dag);
    println!(
        "[{}]\n  pattern:    {:?}\n  confidence: {:.0}%\n  reason:     {}\n",
        label,
        sel.pattern,
        sel.confidence * 100.0,
        sel.reason,
    );
}

fn demo_routing() {
    let mut dag = SubtaskDAG::new("support-ticket");

    let router = dag.add_task(SwarmSubtask::new(
        "ticket_classifier",
        "Read the support ticket and identify its category",
    ));

    let mut billing = SwarmSubtask::new("billing_agent", "Handle billing and payment queries");
    billing.required_capabilities = vec!["billing".into()];
    let b = dag.add_task(billing);

    let mut technical = SwarmSubtask::new("technical_agent", "Handle technical and bug reports");
    technical.required_capabilities = vec!["technical".into()];
    let t = dag.add_task(technical);

    dag.add_dependency(router, b).unwrap();
    dag.add_dependency(router, t).unwrap();

    print_result("Routing  — 1 source + specialists with required_capabilities", &dag);
}

fn demo_supervision() {
    let mut dag = SubtaskDAG::new("prod-deploy");

    let mut deploy = SwarmSubtask::new("deploy_to_prod", "Push release to production cluster");
    deploy.risk_level = RiskLevel::Critical;
    deploy.hitl_required = true;
    let d = dag.add_task(deploy);

    let supervisor = dag.add_task(SwarmSubtask::new(
        "sre_supervisor",
        "Verify deployment health and roll back if needed",
    ));

    dag.add_dependency(d, supervisor).unwrap();

    print_result("Supervision — critical-risk task triggers oversight", &dag);
}

fn demo_debate() {
    let mut dag = SubtaskDAG::new("architecture-choice");

    let pro = dag.add_task(SwarmSubtask::new(
        "microservices_advocate",
        "Argue for microservices architecture",
    ));
    let con = dag.add_task(SwarmSubtask::new(
        "monolith_advocate",
        "Argue for monolithic architecture",
    ));
    let judge = dag.add_task(SwarmSubtask::new(
        "chief_architect",
        "Evaluate both arguments and decide",
    ));

    dag.add_dependency(pro, judge).unwrap();
    dag.add_dependency(con, judge).unwrap();

    print_result("Debate   — exactly 2 sources → 1 judge sink", &dag);
}

fn demo_consensus() {
    let mut dag = SubtaskDAG::new("fraud-detection");

    let caps = vec!["fraud-classifier".into()];
    let make_voter = |id: &str, desc: &str| {
        let mut t = SwarmSubtask::new(id, desc);
        t.required_capabilities = caps.clone();
        t
    };

    let v1 = dag.add_task(make_voter("model_a", "ML model A: classify transaction"));
    let v2 = dag.add_task(make_voter("model_b", "ML model B: classify transaction"));
    let v3 = dag.add_task(make_voter("model_c", "ML model C: classify transaction"));
    let agg = dag.add_task(SwarmSubtask::new(
        "risk_adjudicator",
        "Apply majority vote and issue final ruling",
    ));

    dag.add_dependency(v1, agg).unwrap();
    dag.add_dependency(v2, agg).unwrap();
    dag.add_dependency(v3, agg).unwrap();

    print_result("Consensus — 3 equivalent voters (same caps) → 1 aggregator", &dag);
}

fn demo_mapreduce() {
    let mut dag = SubtaskDAG::new("paper-summarization");

    let s1 = dag.add_task(SwarmSubtask::new("section_1", "Summarise Introduction"));
    let s2 = dag.add_task(SwarmSubtask::new("section_2", "Summarise Methodology"));
    let s3 = dag.add_task(SwarmSubtask::new("section_3", "Summarise Results"));
    let s4 = dag.add_task(SwarmSubtask::new("section_4", "Summarise Conclusion"));
    let red = dag.add_task(SwarmSubtask::new(
        "final_summary",
        "Merge all section summaries into one abstract",
    ));

    dag.add_dependency(s1, red).unwrap();
    dag.add_dependency(s2, red).unwrap();
    dag.add_dependency(s3, red).unwrap();
    dag.add_dependency(s4, red).unwrap();

    print_result("MapReduce — 4 heterogeneous mappers → 1 reducer", &dag);
}

fn demo_sequential() {
    let mut dag = SubtaskDAG::new("ci-pipeline");

    let build = dag.add_task(SwarmSubtask::new("build", "Compile the project"));
    let test = dag.add_task(SwarmSubtask::new("test", "Run the test suite"));
    let lint = dag.add_task(SwarmSubtask::new("lint", "Check code style"));
    let deploy = dag.add_task(SwarmSubtask::new("deploy", "Deploy to staging"));

    dag.add_dependency(build, test).unwrap();
    dag.add_dependency(test, lint).unwrap();
    dag.add_dependency(lint, deploy).unwrap();

    print_result("Sequential — strict linear chain A→B→C→D", &dag);
}

fn demo_parallel() {
    let mut dag = SubtaskDAG::new("translation");

    dag.add_task(SwarmSubtask::new("translate_en", "Translate to English"));
    dag.add_task(SwarmSubtask::new("translate_es", "Translate to Spanish"));
    dag.add_task(SwarmSubtask::new("translate_fr", "Translate to French"));
    dag.add_task(SwarmSubtask::new("translate_de", "Translate to German"));

    print_result("Parallel  — 4 independent tasks, no edges (fallback)", &dag);
}
