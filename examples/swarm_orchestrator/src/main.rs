use futures::future::BoxFuture;
use mofa_kernel::agent::types::error::GlobalResult;
use mofa_foundation::swarm::{
    FailurePolicy, ParallelScheduler, SubtaskDAG, SwarmSchedulerConfig, SwarmSubtask, SwarmScheduler
};
use std::sync::Arc;
use tokio::time::{sleep, Duration};
use tracing::{info, Level};

#[tokio::main]
async fn main() -> GlobalResult<()> {
    // Configure tracing to print to stdout so users can see the logs
    tracing_subscriber::fmt()
        .with_max_level(Level::INFO)
        .init();

    info!("Starting Swarm Orchestrator Example");

    // 1. Build a Directed Acyclic Graph (DAG) for our Swarm Tasks
    let mut dag = SubtaskDAG::new("data_pipeline");

    // Add nodes to the DAG
    let fetch_id = dag.add_task(SwarmSubtask::new("fetch", "Fetch data from external API"));
    let analyze_a_id = dag.add_task(SwarmSubtask::new("analyze_a", "Analyze using Model A"));
    let analyze_b_id = dag.add_task(SwarmSubtask::new("analyze_b", "Analyze using Model B"));
    let summarize_id = dag.add_task(SwarmSubtask::new("summarize", "Summarize the final results"));

    // Add dependencies (Creates a diamond topology)
    // fetch -> {analyze_a, analyze_b} -> summarize
    dag.add_dependency(fetch_id, analyze_a_id).unwrap();
    dag.add_dependency(fetch_id, analyze_b_id).unwrap();
    dag.add_dependency(analyze_a_id, summarize_id).unwrap();
    dag.add_dependency(analyze_b_id, summarize_id).unwrap();

    // 2. Define the Pure Executor Function (The Agent Logic)
    let executor_fn = Arc::new(move |_idx, task: SwarmSubtask| -> BoxFuture<'static, GlobalResult<String>> {
        Box::pin(async move {
            info!("🚀 [Agent Execute] Starting task: {}", task.description);
            // Simulate work via async sleep
            sleep(Duration::from_millis(500)).await;
            info!("✅ [Agent Complete] Finished task: {}", task.description);

            // Return dummy successful result
            Ok(format!("{} result payload", task.id))
        })
    });

    // 3. Configure the Execution Engine
    // We choose the Parallel scheduler to execute branching paths (like analyze_a and analyze_b) concurrently.
    let mut config = SwarmSchedulerConfig::default();
    config.concurrency_limit = Some(2); // Only allow 2 tasks to execute simultaneously
    config.failure_policy = FailurePolicy::FailFastCascade; // If one task fails, abort dependent downstream tasks

    let scheduler = ParallelScheduler::with_config(config);

    // 4. Run the DAG
    info!("Starting Parallel execution engine...");
    let summary = scheduler.execute(&mut dag, executor_fn).await?;

    info!("----------------------------------------");
    info!("Execution Finished!");
    info!("Total Tasks: {}", summary.total_tasks);
    info!("Completed: {}", summary.succeeded);
    info!("Failed: {}", summary.failed);
    info!("Skipped: {}", summary.skipped);

    // 5. Inspect final DAG state
    info!("Final Topology State:");
    for (_idx, task) in dag.all_tasks() {
        info!("  - Task '{}' ({}) is in state: {:?}", task.id, task.description, task.status);
    }

    Ok(())
}
