//! Demonstrates all 5 Phase-2 coordination patterns.
//!
//! Run with: RUST_LOG=info cargo run -p swarm_coordination_patterns

use std::sync::Arc;

use anyhow::Result;
use futures::future::BoxFuture;
use mofa_foundation::swarm::{
    ConsensusScheduler, DebateScheduler, MapReduceScheduler, RoutingScheduler, SubtaskDAG,
    SubtaskExecutorFn, SupervisionScheduler, SwarmScheduler, SwarmSubtask,
};
use mofa_kernel::agent::types::error::{GlobalError, GlobalResult};
use tracing::info;

fn simple_executor() -> SubtaskExecutorFn {
    Arc::new(move |_idx, task: SwarmSubtask| -> BoxFuture<'static, GlobalResult<String>> {
        let id = task.id.clone();
        Box::pin(async move { Ok(format!("{}: done", id)) })
    })
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    run_map_reduce().await?;
    run_debate().await?;
    run_consensus().await?;
    run_routing().await?;
    run_supervision().await?;

    Ok(())
}

async fn run_map_reduce() -> Result<()> {
    // mappers process chunks in parallel; reducer aggregates all outputs
    let mut dag = SubtaskDAG::new("map-reduce");
    let m1 = dag.add_task(SwarmSubtask::new("chunk-1", "Summarize document 1"));
    let m2 = dag.add_task(SwarmSubtask::new("chunk-2", "Summarize document 2"));
    let m3 = dag.add_task(SwarmSubtask::new("chunk-3", "Summarize document 3"));
    let r = dag.add_task(SwarmSubtask::new("reducer", "Merge all summaries"));
    dag.add_dependency(m1, r)?;
    dag.add_dependency(m2, r)?;
    dag.add_dependency(m3, r)?;

    let summary = MapReduceScheduler::new()
        .execute(&mut dag, simple_executor())
        .await?;

    info!(
        pattern = %summary.pattern,
        succeeded = summary.succeeded,
        failed = summary.failed,
        "MapReduce complete"
    );
    println!("[MapReduce]   succeeded={} failed={}", summary.succeeded, summary.failed);
    Ok(())
}

async fn run_debate() -> Result<()> {
    // two debaters argue; judge synthesises the verdict
    let mut dag = SubtaskDAG::new("debate");
    let pro = dag.add_task(SwarmSubtask::new("pro", "Argue for Postgres"));
    let con = dag.add_task(SwarmSubtask::new("con", "Argue for MongoDB"));
    let judge = dag.add_task(SwarmSubtask::new("judge", "Pick the better option"));
    dag.add_dependency(pro, judge)?;
    dag.add_dependency(con, judge)?;

    let summary = DebateScheduler::new()
        .execute(&mut dag, simple_executor())
        .await?;

    info!(pattern = %summary.pattern, succeeded = summary.succeeded, "Debate complete");
    println!("[Debate]      succeeded={} failed={}", summary.succeeded, summary.failed);
    Ok(())
}

async fn run_consensus() -> Result<()> {
    // three voters classify independently; aggregator picks majority
    let mut dag = SubtaskDAG::new("consensus");
    let v1 = dag.add_task(SwarmSubtask::new("voter-1", "Classify sentiment"));
    let v2 = dag.add_task(SwarmSubtask::new("voter-2", "Classify sentiment"));
    let v3 = dag.add_task(SwarmSubtask::new("voter-3", "Classify sentiment"));
    let agg = dag.add_task(SwarmSubtask::new("aggregator", "Pick majority verdict"));
    dag.add_dependency(v1, agg)?;
    dag.add_dependency(v2, agg)?;
    dag.add_dependency(v3, agg)?;

    let summary = ConsensusScheduler::new()
        .execute(&mut dag, simple_executor())
        .await?;

    info!(pattern = %summary.pattern, succeeded = summary.succeeded, "Consensus complete");
    println!("[Consensus]   succeeded={} failed={}", summary.succeeded, summary.failed);
    Ok(())
}

async fn run_routing() -> Result<()> {
    // router signals "database"; only the db-specialist runs; ml-specialist is skipped
    let mut dag = SubtaskDAG::new("routing");
    let router = dag.add_task(SwarmSubtask::new("router", "Identify required capability"));

    let mut db = SwarmSubtask::new("db-specialist", "Handle database queries");
    db.required_capabilities = vec!["database".into()];
    let idx_db = dag.add_task(db);

    let mut ml = SwarmSubtask::new("ml-specialist", "Handle ML inference");
    ml.required_capabilities = vec!["machine_learning".into()];
    let idx_ml = dag.add_task(ml);

    dag.add_dependency(router, idx_db)?;
    dag.add_dependency(router, idx_ml)?;

    let routing_executor: SubtaskExecutorFn =
        Arc::new(move |_idx, task: SwarmSubtask| -> BoxFuture<'static, GlobalResult<String>> {
            let id = task.id.clone();
            Box::pin(async move {
                if id == "router" {
                    Ok("needs database access".into())
                } else {
                    Ok(format!("{}: done", id))
                }
            })
        });

    let summary = RoutingScheduler::new()
        .execute(&mut dag, routing_executor)
        .await?;

    info!(
        pattern = %summary.pattern,
        succeeded = summary.succeeded,
        skipped = summary.skipped,
        "Routing complete"
    );
    println!(
        "[Routing]     succeeded={} skipped={}",
        summary.succeeded, summary.skipped
    );
    Ok(())
}

async fn run_supervision() -> Result<()> {
    // worker-b crashes; supervisor always runs and receives failure context
    let mut dag = SubtaskDAG::new("supervision");
    let w1 = dag.add_task(SwarmSubtask::new("worker-a", "Process shard A"));
    let w2 = dag.add_task(SwarmSubtask::new("worker-b", "Process shard B"));
    let sup = dag.add_task(SwarmSubtask::new("supervisor", "Review worker results"));
    dag.add_dependency(w1, sup)?;
    dag.add_dependency(w2, sup)?;

    let supervision_executor: SubtaskExecutorFn =
        Arc::new(move |_idx, task: SwarmSubtask| -> BoxFuture<'static, GlobalResult<String>> {
            let id = task.id.clone();
            Box::pin(async move {
                if id == "worker-b" {
                    Err(GlobalError::runtime("shard B unavailable"))
                } else {
                    Ok(format!("{}: done", id))
                }
            })
        });

    let summary = SupervisionScheduler::new()
        .execute(&mut dag, supervision_executor)
        .await?;

    info!(
        pattern = %summary.pattern,
        succeeded = summary.succeeded,
        failed = summary.failed,
        "Supervision complete"
    );
    println!(
        "[Supervision] succeeded={} failed={}",
        summary.succeeded, summary.failed
    );
    Ok(())
}
