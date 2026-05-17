use mofa_foundation::swarm::{
    AgentSpec, SubtaskDAG, SwarmCapabilityRegistry, SwarmSubtask,
};

fn main() {
    let registry = SwarmCapabilityRegistry::new()
        .register(AgentSpec {
            id: "summarizer-a".into(),
            capabilities: vec!["summarize".into()],
            model: Some("gpt-4o-mini".into()),
            cost_per_token: Some(0.00015),
            max_concurrency: 4,
        })
        .register(AgentSpec {
            id: "summarizer-b".into(),
            capabilities: vec!["summarize".into()],
            model: Some("claude-3-haiku".into()),
            cost_per_token: Some(0.00025),
            max_concurrency: 4,
        })
        .register(AgentSpec {
            id: "translator".into(),
            capabilities: vec!["translate".into()],
            model: Some("gpt-4o".into()),
            cost_per_token: Some(0.005),
            max_concurrency: 2,
        })
        .register(AgentSpec {
            id: "full-stack".into(),
            capabilities: vec!["summarize".into(), "translate".into(), "review".into()],
            model: Some("claude-3-5-sonnet".into()),
            cost_per_token: Some(0.003),
            max_concurrency: 8,
        });

    println!("registered {} agents, {} capabilities\n", registry.agent_count(), registry.capability_count());

    // build a dag with 5 tasks that have different capability requirements
    let mut dag = SubtaskDAG::new("research-pipeline");
    dag.add_task(SwarmSubtask::new("ingest", "ingest source documents"));
    dag.add_task(
        SwarmSubtask::new("summarize-en", "summarize in english")
            .with_capabilities(vec!["summarize".into()]),
    );
    dag.add_task(
        SwarmSubtask::new("translate-fr", "translate to french")
            .with_capabilities(vec!["translate".into()]),
    );
    dag.add_task(
        SwarmSubtask::new("summarize-and-translate", "summarize then translate")
            .with_capabilities(vec!["summarize".into(), "translate".into()]),
    );
    dag.add_task(
        SwarmSubtask::new("code-review", "review generated code")
            .with_capabilities(vec!["code-analysis".into()]),
    );

    // coverage report before execution
    let report = registry.coverage_report(&dag);

    println!("coverage report:");
    println!("  covered ({})  : {:?}", report.covered.len(), report.covered);
    println!("  partial ({})  : {:?} (single point of failure)", report.partial.len(), report.partial);
    println!("  uncovered ({}) : {:?} (will fail at dispatch)", report.uncovered.len(), report.uncovered);
    println!("  gaps         : {:?}", report.gaps);
    println!();
    println!("is fully covered : {}", report.is_fully_covered());
    println!("has spof risk    : {}", report.has_spof_risk());
    println!("problem count    : {}", report.problem_count());
    println!();

    // find agents for the multi-capability task
    let task = SwarmSubtask::new("q", "multi-cap query")
        .with_capabilities(vec!["summarize".into(), "translate".into()]);
    let candidates = registry.find_for_task(&task);
    println!("agents capable of [summarize + translate]:");
    for a in &candidates {
        println!("  {} (model: {:?})", a.id, a.model);
    }
    println!();

    // single capability lookup
    let summarizers = registry.find_by_capability("summarize");
    println!("all summarizers ({}):", summarizers.len());
    for a in &summarizers {
        println!("  {}", a.id);
    }
}
