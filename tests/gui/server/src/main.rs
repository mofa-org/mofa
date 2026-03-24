//! HTTP backend for the swarm artifact viewer.
//!
//! The server exposes:
//! - `POST /swarm/run` to execute the demo incident-response swarm
//! - `GET /swarm/artifact` to return the latest generated `SwarmRunArtifact`

use std::sync::Arc;
use std::time::Duration;

use axum::{
    extract::State,
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use futures::future::BoxFuture;
use mofa_foundation::swarm::{
    AuditEvent, AuditEventKind, CoordinationPattern, ParallelScheduler, RiskLevel, SubtaskDAG,
    SwarmMetrics, SwarmResult, SwarmScheduler, SwarmStatus, SwarmSubtask,
};
use mofa_kernel::agent::types::error::GlobalResult;
use mofa_testing::SwarmRunArtifact;
use serde_json::{json, Value};
use tokio::sync::RwLock;
use tower_http::cors::{Any, CorsLayer};

/// Start the local HTTP server that powers the swarm artifact viewer.
#[tokio::main]
async fn main() {
    // Keep only the latest artifact in memory; the GUI can fetch it after a run completes.
    let state = Arc::new(RwLock::new(None::<Value>));

    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods([axum::http::Method::GET, axum::http::Method::POST])
        .allow_headers(Any);

    let app = Router::new()
        .route("/swarm/run", post(run_swarm))
        .route("/swarm/artifact", get(get_artifact))
        .with_state(state)
        .layer(cors);

    let addr = "127.0.0.1:3001";
    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .expect("failed to bind server address");

    println!("Swarm artifact server listening on http://{addr}");

    axum::serve(listener, app)
        .await
        .expect("server failed");
}

/// Execute the demo swarm and return the freshly generated artifact payload.
async fn run_swarm(State(state): State<Arc<RwLock<Option<Value>>>>) -> impl IntoResponse {
    match build_demo_artifact().await {
        Ok(value) => {
            // Persist the most recent run so `GET /swarm/artifact` can serve it back.
            *state.write().await = Some(value.clone());
            (StatusCode::OK, Json(value)).into_response()
        }
        Err(err) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": err.to_string() })),
        )
            .into_response(),
    }
}

/// Return the most recently generated artifact without running the swarm again.
async fn get_artifact(State(state): State<Arc<RwLock<Option<Value>>>>) -> impl IntoResponse {
    let artifact = state.read().await.clone();
    match artifact {
        Some(value) => (StatusCode::OK, Json(value)).into_response(),
        None => (
            StatusCode::NOT_FOUND,
            Json(json!({ "error": "no artifact available" })),
        )
            .into_response(),
    }
}

/// Build the incident-response demo DAG, execute it, and convert it into a rich test artifact.
async fn build_demo_artifact() -> GlobalResult<Value> {
    let mut dag = SubtaskDAG::new("incident-response");

    let triage = dag.add_task(
        SwarmSubtask::new("triage", "Triage incoming incident")
            .with_capabilities(vec!["triage".into()]),
    );
    let investigate = dag.add_task(
        SwarmSubtask::new("investigate", "Investigate impact")
            .with_capabilities(vec!["analysis".into()]),
    );
    let remediate = dag.add_task(
        SwarmSubtask::new("remediate", "Apply remediation")
            .with_capabilities(vec!["ops".into()])
            .with_risk_level(RiskLevel::High),
    );
    let report = dag.add_task(
        SwarmSubtask::new("report", "Publish summary")
            .with_capabilities(vec!["reporting".into()]),
    );

    dag.add_dependency(triage, investigate)?;
    dag.add_dependency(investigate, remediate)?;
    dag.add_dependency(remediate, report)?;

    dag.assign_agent(triage, "router");
    dag.assign_agent(investigate, "analyst");
    dag.assign_agent(remediate, "operator");
    dag.assign_agent(report, "communicator");

    let executor = Arc::new(|_idx, task: SwarmSubtask| -> BoxFuture<'static, _> {
        Box::pin(async move {
            tokio::time::sleep(Duration::from_millis(15)).await;
            Ok(format!(
                "{} handled by {}",
                task.id,
                task.assigned_agent.unwrap_or_else(|| "unassigned".into())
            ))
        })
    });

    let scheduler = ParallelScheduler::new();
    let summary = scheduler.execute(&mut dag, executor).await?;

    // Build a richer artifact payload than the raw scheduler summary so the GUI can show metrics.
    let mut metrics = SwarmMetrics::default();
    for _ in 0..summary.succeeded {
        metrics.record_task_completed();
    }
    for _ in 0..summary.failed {
        metrics.record_task_failed();
    }
    metrics.record_agent_tokens("router", 42);
    metrics.record_agent_tokens("analyst", 84);
    metrics.record_agent_tokens("operator", 96);
    metrics.record_agent_tokens("communicator", 33);
    metrics.set_duration_ms(summary.total_wall_time.as_millis() as u64);

    // Seed a small audit trail so the UI can exercise event rendering end to end.
    let audit_events = vec![
        AuditEvent::new(AuditEventKind::SwarmStarted, "Incident-response swarm started")
            .with_data(json!({"swarm_id":"incident-response"})),
        AuditEvent::new(AuditEventKind::AgentAssigned, "Agents assigned to subtasks").with_data(
            json!({
                "router":"triage",
                "analyst":"investigate",
                "operator":"remediate",
                "communicator":"report"
            }),
        ),
        AuditEvent::new(
            AuditEventKind::SwarmCompleted,
            "Incident-response swarm completed successfully",
        )
        .with_data(json!({"swarm_id":"incident-response","succeeded":summary.succeeded})),
    ];

    let result = SwarmResult {
        config_id: "demo-incident-response".into(),
        status: SwarmStatus::Completed,
        dag,
        output: Some("Incident triaged, remediated, and reported".into()),
        metrics,
        audit_events,
        started_at: chrono::Utc::now(),
        completed_at: Some(chrono::Utc::now()),
    };
    // Reuse the testing artifact constructor so GUI output matches test/CI artifact semantics.
    let artifact = SwarmRunArtifact::from_swarm_result(
        &result,
        CoordinationPattern::Parallel,
        Some(&summary),
    );

    Ok(serde_json::to_value(artifact).expect("artifact serialization should succeed"))
}
