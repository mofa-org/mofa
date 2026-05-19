//! Debate: Frontend Framework Selection
//!
//! Scenario: Two engineers advocate for React and Vue respectively.
//! The CTO reads both arguments and picks the frontend framework
//! for the next product generation.
//!
//! Run: RUST_LOG=info cargo run -p swarm_coordination_patterns --bin debate_tech_stack

use std::sync::Arc;

use anyhow::Result;
use futures::future::BoxFuture;
use mofa_foundation::swarm::{
    DebateScheduler, SubtaskDAG, SubtaskExecutorFn, SwarmScheduler, SwarmSubtask,
};
use mofa_kernel::agent::types::error::GlobalResult;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    println!("=== Debate: Frontend Framework Selection ===");
    println!("DAG: react_advocate, vue_advocate (parallel) -> cto\n");

    let mut dag = SubtaskDAG::new("framework-debate");

    let react = dag.add_task(SwarmSubtask::new("react_advocate", "Argue why React is the best choice for the new frontend"));
    let vue   = dag.add_task(SwarmSubtask::new("vue_advocate",   "Argue why Vue is the best choice for the new frontend"));
    let cto   = dag.add_task(SwarmSubtask::new("cto",            "Evaluate both arguments and select the frontend framework"));

    dag.add_dependency(react, cto)?;
    dag.add_dependency(vue, cto)?;

    let executor: SubtaskExecutorFn =
        Arc::new(move |_idx, task: SwarmSubtask| -> BoxFuture<'static, GlobalResult<String>> {
            let id = task.id.clone();
            let desc = task.description.clone();
            Box::pin(async move {
                let out = match id.as_str() {
                    "react_advocate" => "REACT: largest ecosystem (npm downloads 4x Vue), team already has 3 React engineers, Next.js gives SSR out of the box, React Native reuse for mobile, hiring pool 2x larger".to_string(),
                    "vue_advocate"   => "VUE: gentler learning curve (faster onboarding), single-file components reduce context switching, Nuxt.js equally capable SSR, smaller bundle baseline, better TypeScript DX in Vue 3 Composition API".to_string(),
                    "cto" => {
                        let has_context = desc.contains("Debate Arguments");
                        format!(
                            "framework_decision: REACT — {} Existing team expertise and hiring market depth outweigh Vue's DX advantage. Adopt Next.js 14 App Router. Run Vue skill-share session for team awareness.",
                            if has_context { "both arguments considered." } else { "no context." }
                        )
                    }
                    _ => "done".to_string(),
                };
                Ok(out.to_string())
            })
        });

    let summary = DebateScheduler::new().execute(&mut dag, executor).await?;

    println!("Advocates (ran in parallel):");
    for r in &summary.results {
        if r.task_id == "cto" { continue; }
        let status = if r.outcome.is_success() { "OK" } else { "FAIL" };
        println!("  [{}] {} -> {}", status, r.task_id, r.outcome.output().unwrap_or("(no output)"));
    }

    if let Some(r) = summary.results.iter().find(|r| r.task_id == "cto") {
        let status = if r.outcome.is_success() { "OK" } else { "FAIL" };
        println!("\nCTO decision (receives both arguments injected):\n  [{}] {} -> {}",
            status, r.task_id, r.outcome.output().unwrap_or("(no output)"));
    }

    println!("\nsucceeded={} failed={} total_time={:?}",
        summary.succeeded, summary.failed, summary.total_wall_time);

    Ok(())
}
