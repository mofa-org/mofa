//! Demonstrates agent capability manifest registration and runtime discovery.
//!
//! Two specialized agents register their manifests with a `CapabilityRegistry`.
//! An orchestrator then queries the registry to route three different tasks to
//! the correct agent automatically, without any hardcoded agent references.

use mofa_foundation::CapabilityRegistry;
use mofa_kernel::{
    AgentManifest,
    agent::capabilities::AgentCapabilities,
    agent::config::schema::CoordinationMode,
};

fn main() {
    let mut registry = CapabilityRegistry::new();

    // --- agent registration ---

    // ResearchAgent: specializes in web search and document summarization
    registry.register(
        AgentManifest::builder("agent-research", "ResearchAgent")
            .description("searches the web and summarizes documents and articles")
            .capabilities(
                AgentCapabilities::builder()
                    .with_tag("research")
                    .with_tag("summarization")
                    .with_tag("web")
                    .build(),
            )
            .tool("web_search")
            .tool("document_reader")
            .memory_backend("in-memory")
            .coordination_mode(CoordinationMode::Sequential)
            .build(),
    );

    // CodeAgent: specializes in writing and reviewing Rust code
    registry.register(
        AgentManifest::builder("agent-code", "CodeAgent")
            .description("writes reviews and debugs Rust and Python code")
            .capabilities(
                AgentCapabilities::builder()
                    .with_tag("coding")
                    .with_tag("rust")
                    .with_tag("python")
                    .with_tag("review")
                    .build(),
            )
            .tool("code_linter")
            .tool("test_runner")
            .memory_backend("in-memory")
            .coordination_mode(CoordinationMode::Sequential)
            .build(),
    );

    println!("registered {} agents\n", registry.len());

    // --- orchestrator routing ---

    let tasks = [
        "summarize recent web articles about AI",
        "write a Rust function to parse JSON",
        "review this Python code for bugs",
    ];

    for task in &tasks {
        println!("task: \"{}\"", task);
        let results = registry.query(task);
        if let Some(best) = results.first() {
            println!(
                "  routed to: {} ({})\n  description: {}\n  tools: {:?}\n",
                best.name, best.agent_id, best.description, best.tools
            );
        } else {
            println!("  no matching agent found\n");
        }
    }

    // --- direct tag lookup ---

    println!("agents with 'coding' tag:");
    for m in registry.find_by_tag("coding") {
        println!("  {} — {}", m.name, m.description);
    }
}
