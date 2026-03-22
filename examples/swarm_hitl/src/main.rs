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
use opentelemetry::global;
use opentelemetry::trace::TracerProvider as _;
use opentelemetry_otlp::WithExportConfig;
use opentelemetry_sdk::{trace as sdktrace, runtime, Resource};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::sync::Mutex;
use tracing::{info, warn, Instrument};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

use mofa_foundation::swarm::{
    FailurePolicy, HITLMode, ParallelScheduler, SubtaskDAG, SubtaskExecutorFn,
    SwarmScheduler, SwarmSchedulerConfig, SwarmSubtask, hitl_executor_middleware,
};
use mofa_foundation::swarm::hitl::{ApprovalOutcome, ChannelApprovalHandler};

async fn init_tracer() -> Result<sdktrace::Tracer, Box<dyn std::error::Error>> {
    let exporter = opentelemetry_otlp::SpanExporter::builder()
        .with_http()
        .with_endpoint("http://localhost:4318/v1/traces")
        .build()?;

    let provider = sdktrace::TracerProvider::builder()
        .with_resource(Resource::new(vec![
            opentelemetry::KeyValue::new("service.name", "swarm-hitl"),
        ]))
        .with_batch_exporter(exporter, runtime::Tokio)
        .build();

    let tracer = provider.tracer("swarm-hitl");
    global::set_tracer_provider(provider);
    Ok(tracer)
}

#[tokio::main]
async fn main() {
    let tracer = match init_tracer().await {
        Ok(tracer) => Some(tracer),
        Err(e) => {
            eprintln!("Warning: Jaeger not available ({e}), running without OTel traces");
            None
        }
    };

    let otel_layer = tracer.map(|tracer| tracing_opentelemetry::layer().with_tracer(tracer));
    tracing_subscriber::registry()
        .with(EnvFilter::new("info"))
        .with(tracing_subscriber::fmt::layer())
        .with(otel_layer)
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
        let task_id = task.id.clone();
        let task_description = task.description.clone();
        let risk_level = format!("{:?}", task.risk_level);
        let hitl_required = task.hitl_required;
        Box::pin(async move {
            let span = tracing::info_span!(
                "swarm.subtask.execute",
                subtask_id = %task_id,
                description = %task_description,
                risk_level = %risk_level,
                hitl_required = hitl_required,
            );

            async move {
                info!("[Agent] Running: '{}'", task.description);
                tokio::time::sleep(tokio::time::Duration::from_millis(400)).await;
                let output = format!("[result of: {}]", task.id);
                info!("[Agent] Done: '{}' => {}", task.id, output);
                Ok(output)
            }
            .instrument(span)
            .await
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
            println!("  Risk    : {:?}", req.risk_level);
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

    info!("Starting parallel DAG execution with HITL gates...");
    let scheduler = ParallelScheduler::with_config(config);
    let dag_task_count = dag.task_count();
    let summary = scheduler
        .execute(&mut dag, hitl_executor)
        .instrument(tracing::info_span!(
            "swarm_hitl_run",
            dag_task_count = dag_task_count,
            hitl_mode = "required",
        ))
        .await
        .unwrap();

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

    // Flush all pending OTel spans to Jaeger before exit.
    global::shutdown_tracer_provider();
    println!("Open http://localhost:16686 — search service: swarm-hitl");
}
