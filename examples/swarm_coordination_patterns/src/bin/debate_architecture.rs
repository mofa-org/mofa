use std::sync::Arc;

use anyhow::Result;
use futures::future::BoxFuture;
use mofa_foundation::swarm::{
    DebateScheduler, SubtaskDAG, SubtaskExecutorFn, SwarmScheduler, SwarmSubtask,
};
use mofa_kernel::agent::types::error::GlobalResult;

fn debate_executor() -> SubtaskExecutorFn {
    Arc::new(move |_idx, task: SwarmSubtask| -> BoxFuture<'static, GlobalResult<String>> {
        let id = task.id.clone();
        let desc = task.description.clone();
        Box::pin(async move {
            let output = match id.as_str() {
                "microservices_advocate" => {
                    "microservices_argument: Independent deployability reduces blast radius, \
                     polyglot persistence enables best-fit storage per domain, \
                     horizontal scaling per service optimizes cost under variable load".to_string()
                }
                "monolith_advocate" => {
                    "monolith_argument: Single deployment unit eliminates distributed systems \
                     overhead, shared memory transactions are simpler than saga patterns, \
                     developer onboarding time is 3x faster with one codebase".to_string()
                }
                "chief_architect" => {
                    let received_context = if desc.contains("Debate Arguments") {
                        "both sides received"
                    } else {
                        "no context"
                    };
                    format!(
                        "verdict: Given team size <10 and MVP timeline, monolith is recommended. \
                         Revisit microservices at >50k DAU. Context: {}.",
                        received_context
                    )
                }
                _ => format!("{}: processed", id),
            };
            Ok(output)
        })
    })
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let mut dag = SubtaskDAG::new("architecture-debate");

    let micro = dag.add_task(SwarmSubtask::new(
        "microservices_advocate",
        "Argue for microservices architecture",
    ));
    let mono = dag.add_task(SwarmSubtask::new(
        "monolith_advocate",
        "Argue for monolithic architecture",
    ));
    let judge = dag.add_task(SwarmSubtask::new(
        "chief_architect",
        "Synthesize both arguments and issue architecture decision",
    ));

    dag.add_dependency(micro, judge)?;
    dag.add_dependency(mono, judge)?;

    println!("=== Debate: Microservices vs Monolith Architecture Decision ===");
    println!("DAG: microservices_advocate, monolith_advocate (parallel) -> chief_architect\n");

    let summary = DebateScheduler::new()
        .execute(&mut dag, debate_executor())
        .await?;

    let debaters: Vec<_> = summary.results.iter()
        .filter(|r| r.task_id != "chief_architect")
        .collect();
    let judge_result = summary.results.iter().find(|r| r.task_id == "chief_architect");

    println!("Debaters (ran in parallel):");
    for r in &debaters {
        let status = if r.outcome.is_success() { "OK" } else { "FAIL" };
        println!("  [{}] {} ->\n    {}", status, r.task_id, r.outcome.output().unwrap_or("(no output)"));
    }

    if let Some(r) = judge_result {
        let status = if r.outcome.is_success() { "OK" } else { "FAIL" };
        println!("\nJudge (receives both arguments injected into description):");
        println!("  [{}] {} ->\n    {}", status, r.task_id, r.outcome.output().unwrap_or("(no output)"));
    }

    println!("\nSummary: succeeded={} failed={} total_wall_time={:?}",
        summary.succeeded, summary.failed, summary.total_wall_time);

    Ok(())
}
