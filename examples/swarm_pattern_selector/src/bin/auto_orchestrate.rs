//! Full cognitive loop: DAG topology → PatternSelector → execute.
//!
//! No pattern is chosen manually. The selector reads the DAG shape and picks
//! the right scheduler automatically. Three scenarios are shown back-to-back.
//!
//! Run: RUST_LOG=info cargo run -p swarm_pattern_selector --bin auto_orchestrate

use std::sync::Arc;

use anyhow::Result;
use futures::future::BoxFuture;
use mofa_foundation::swarm::{
    PatternSelector, RiskLevel, SubtaskDAG, SubtaskExecutorFn, SwarmSubtask,
};
use mofa_kernel::agent::types::error::GlobalResult;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    println!("=== Auto-Orchestrated Swarm: select → execute, zero manual pattern choice ===\n");

    scenario_mapreduce().await?;
    scenario_debate().await?;
    scenario_supervision().await?;

    println!("=== All scenarios complete ===");
    Ok(())
}

fn simple_executor(prefix: &'static str) -> SubtaskExecutorFn {
    Arc::new(move |_idx, task: SwarmSubtask| -> BoxFuture<'static, GlobalResult<String>> {
        let id = task.id.clone();
        Box::pin(async move { Ok(format!("{prefix}[{id}]: done")) })
    })
}

async fn scenario_mapreduce() -> Result<()> {
    println!("--- Scenario 1: Research paper summarisation ---");

    let mut dag = SubtaskDAG::new("paper-summary");
    let s1 = dag.add_task(SwarmSubtask::new("section_intro", "Summarise Introduction"));
    let s2 = dag.add_task(SwarmSubtask::new("section_methods", "Summarise Methodology"));
    let s3 = dag.add_task(SwarmSubtask::new("section_results", "Summarise Results"));
    let s4 = dag.add_task(SwarmSubtask::new("section_conclusion", "Summarise Conclusion"));
    let red = dag.add_task(SwarmSubtask::new("final_abstract", "Merge all section summaries"));
    dag.add_dependency(s1, red)?;
    dag.add_dependency(s2, red)?;
    dag.add_dependency(s3, red)?;
    dag.add_dependency(s4, red)?;

    let sel = PatternSelector::select_with_reason(&dag);
    println!(
        "PatternSelector → {:?}  ({:.0}% confidence)\nReason: {}\n",
        sel.pattern,
        sel.confidence * 100.0,
        sel.reason
    );

    let scheduler = sel.pattern.into_scheduler();
    let summary = scheduler
        .execute(&mut dag, simple_executor("summary::"))
        .await?;

    for r in &summary.results {
        println!("  [{}] {}", if r.outcome.is_success() { "OK" } else { "FAIL" }, r.task_id);
    }
    println!(
        "succeeded={} failed={} wall={:?}\n",
        summary.succeeded, summary.failed, summary.total_wall_time
    );
    Ok(())
}

async fn scenario_debate() -> Result<()> {
    println!("--- Scenario 2: Architecture decision ---");

    let mut dag = SubtaskDAG::new("arch-decision");
    let pro = dag.add_task(SwarmSubtask::new("microservices_advocate", "Argue for microservices"));
    let con = dag.add_task(SwarmSubtask::new("monolith_advocate", "Argue for monolith"));
    let judge = dag.add_task(SwarmSubtask::new("chief_architect", "Decide based on both arguments"));
    dag.add_dependency(pro, judge)?;
    dag.add_dependency(con, judge)?;

    let sel = PatternSelector::select_with_reason(&dag);
    println!(
        "PatternSelector → {:?}  ({:.0}% confidence)\nReason: {}\n",
        sel.pattern,
        sel.confidence * 100.0,
        sel.reason
    );

    let scheduler = sel.pattern.into_scheduler();
    let summary = scheduler
        .execute(&mut dag, simple_executor("debate::"))
        .await?;

    for r in &summary.results {
        println!("  [{}] {}", if r.outcome.is_success() { "OK" } else { "FAIL" }, r.task_id);
    }
    println!(
        "succeeded={} failed={} wall={:?}\n",
        summary.succeeded, summary.failed, summary.total_wall_time
    );
    Ok(())
}

async fn scenario_supervision() -> Result<()> {
    println!("--- Scenario 3: Production deployment (high-risk → supervision) ---");

    let mut dag = SubtaskDAG::new("prod-deploy");

    let mut deploy = SwarmSubtask::new("deploy_prod", "Push release to production");
    deploy.risk_level = RiskLevel::Critical;
    deploy.hitl_required = true;
    let d = dag.add_task(deploy);

    let mut health = SwarmSubtask::new("health_check", "Verify cluster health post-deploy");
    health.risk_level = RiskLevel::High;
    let h = dag.add_task(health);

    let supervisor = dag.add_task(SwarmSubtask::new(
        "sre_on_call",
        "Review deployment outcome and approve or roll back",
    ));
    dag.add_dependency(d, supervisor)?;
    dag.add_dependency(h, supervisor)?;

    let sel = PatternSelector::select_with_reason(&dag);
    println!(
        "PatternSelector → {:?}  ({:.0}% confidence)\nReason: {}\n",
        sel.pattern,
        sel.confidence * 100.0,
        sel.reason
    );

    let scheduler = sel.pattern.into_scheduler();
    let summary = scheduler
        .execute(&mut dag, simple_executor("deploy::"))
        .await?;

    for r in &summary.results {
        println!("  [{}] {}", if r.outcome.is_success() { "OK" } else { "FAIL" }, r.task_id);
    }
    println!(
        "succeeded={} failed={} wall={:?}\n",
        summary.succeeded, summary.failed, summary.total_wall_time
    );
    Ok(())
}
