//! Swarm HITL
//!
//! Demonstrates how to wire `hitl_executor_middleware` into a `ParallelScheduler`
//! to pause execution and ask a human to approve each task before it runs.
//!
//! Topology (diamond DAG):
//!
//!                  fetch_data
//!                 /          \
//!          analyze_a       analyze_b   <- parallel HITL gates
//!                 \          /
//!                   report
//!
//! fetch_data runs first (sequential), then analyze_a and analyze_b run in
//! parallel — each pausing independently for human approval — and finally
//! report runs once both complete.
//!
//! Run:
//!   cargo run -p swarm_hitl

use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::sync::Mutex;
use tracing::{info, warn, Level};

use mofa_foundation::swarm::{
    FailurePolicy, HITLMode, ParallelScheduler, SubtaskDAG, SubtaskExecutorFn,
    SwarmScheduler, SwarmSchedulerConfig, SwarmSubtask, hitl_executor_middleware,
};
use mofa_foundation::swarm::hitl::{ApprovalOutcome, ChannelApprovalHandler};

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_max_level(Level::INFO)
        .init();

    info!("=== Swarm HITL (Diamond DAG + ParallelScheduler) ===");
    info!("Each task pauses for human approval before running.");
    info!("analyze_a and analyze_b will ask in parallel.");
    info!("");

    // 1. Build the diamond DAG
    //    fetch_data -> { analyze_a, analyze_b } -> report
    let mut dag = SubtaskDAG::new("research_pipeline");

    let fetch_id     = dag.add_task(SwarmSubtask::new("fetch_data", "Fetch raw market data from API"));
    let analyze_a_id = dag.add_task(SwarmSubtask::new("analyze_a",  "Analyse bullish signals (Model A)"));
    let analyze_b_id = dag.add_task(SwarmSubtask::new("analyze_b",  "Analyse bearish signals (Model B)"));
    let report_id    = dag.add_task(SwarmSubtask::new("report",      "Synthesise findings into report"));

    dag.add_dependency(fetch_id,     analyze_a_id).unwrap();
    dag.add_dependency(fetch_id,     analyze_b_id).unwrap();
    dag.add_dependency(analyze_a_id, report_id).unwrap();
    dag.add_dependency(analyze_b_id, report_id).unwrap();

    // 2. Base executor (simulates agent work with a short sleep)
    let base_executor: SubtaskExecutorFn = Arc::new(|_idx, task| {
        Box::pin(async move {
            info!("[Agent] Running: '{}'", task.description);
            tokio::time::sleep(tokio::time::Duration::from_millis(400)).await;
            let output = format!("[result of: {}]", task.id);
            info!("[Agent] Done: '{}' => {}", task.id, output);
            Ok(output)
        })
    });

    // 3. Terminal reviewer — handles approval requests one at a time
    //    (in a real deployment, this would be a REST API or Observatory UI)
    let (handler, mut rx) = ChannelApprovalHandler::new(8);

    tokio::spawn(async move {
        let stdin = tokio::io::stdin();
        let mut reader = BufReader::new(stdin).lines();

        while let Some((req, reply)) = rx.recv().await {
            println!("\n-------------------------------------------");
            println!("HITL Review Required");
            println!("  Task    : {} — {}", req.subtask_id, req.description);
            println!("  Risk    : {:.1}", req.risk_level);
            println!("-------------------------------------------");
            println!("  (y) Approve  (n) Reject  (m <text>) Modify prompt");
            print!("  > ");

            use std::io::Write;
            std::io::stdout().flush().ok();

            let line = reader.next_line().await
                .ok()
                .flatten()
                .unwrap_or_default();
            let trimmed = line.trim();

            let decision = if trimmed.eq_ignore_ascii_case("y") {
                println!("  Approved");
                ApprovalOutcome::approve()
            } else if trimmed.eq_ignore_ascii_case("n") {
                println!("  Rejected");
                ApprovalOutcome::reject("denied by reviewer")
            } else if let Some(rest) = trimmed.strip_prefix("m ") {
                // "m <text>" — inline modify
                println!("  Modified to: '{}'", rest);
                ApprovalOutcome::modify(rest.to_string())
            } else if trimmed.eq_ignore_ascii_case("m") {
                // bare "m" — ask for the new prompt on the next line
                print!("  New prompt > ");
                use std::io::Write;
                std::io::stdout().flush().ok();
                let new_text = reader.next_line().await
                    .ok().flatten().unwrap_or_default();
                let new_text = new_text.trim().to_string();
                println!("  Modified to: '{}'", new_text);
                ApprovalOutcome::modify(new_text)
            } else {
                warn!("  Unrecognized '{}' — defaulting to approve", trimmed);
                ApprovalOutcome::approve()
            };


            reply.send(decision).ok();
        }
    });

    // 4. Wrap the base executor with the HITL gate
    //    This same hitl_executor works identically if you swap in SequentialScheduler.
    let audit_log = Arc::new(Mutex::new(vec![]));
    let config = SwarmSchedulerConfig {
        concurrency_limit: Some(2),
        failure_policy: FailurePolicy::FailFastCascade,
        ..Default::default()
    };
    let hitl_executor = hitl_executor_middleware(
        base_executor,
        HITLMode::Required,
        Arc::new(handler),
        audit_log.clone(),
        config.hitl_optional_timeout,
    );

    // 5. Run with ParallelScheduler (concurrency_limit = 2 for analyze_a and analyze_b)
    info!("Starting parallel DAG execution with HITL gates...");
    let scheduler = ParallelScheduler::with_config(config);
    let summary = scheduler.execute(&mut dag, hitl_executor).await.unwrap();

    // 6. Results
    println!("\n===========================================");
    println!("  Execution Summary");
    println!("===========================================");
    println!("  Total   : {}", summary.total_tasks);
    println!("  Done    : {}", summary.succeeded);
    println!("  Failed  : {}", summary.failed);
    println!("  Skipped : {}", summary.skipped);
    println!("  Time    : {:?}", summary.total_wall_time);
    println!("===========================================");

    let audit = audit_log.lock().await;
    println!("\n  Audit Log ({} events):", audit.len());
    for event in audit.iter() {
        println!("  [{:?}] {}", event.kind, event.description);
    }
    println!();
}
