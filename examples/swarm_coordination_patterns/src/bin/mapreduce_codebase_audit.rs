//! MapReduce: Codebase Security Audit
//!
//! Scenario: Four module auditors scan different areas of the codebase
//! in parallel (map phase) looking for vulnerabilities. The reducer
//! compiles all findings into a single prioritised security report.
//!
//! Run: RUST_LOG=info cargo run -p swarm_coordination_patterns --bin mapreduce_codebase_audit

use std::sync::Arc;

use anyhow::Result;
use futures::future::BoxFuture;
use mofa_foundation::swarm::{
    MapReduceScheduler, SubtaskDAG, SubtaskExecutorFn, SwarmScheduler, SwarmSubtask,
};
use mofa_kernel::agent::types::error::GlobalResult;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    println!("=== MapReduce: Codebase Security Audit ===");
    println!("DAG: audit_auth, audit_api, audit_db, audit_deps (mappers) -> security_report (reducer)\n");

    let mut dag = SubtaskDAG::new("security-audit");

    let auth = dag.add_task(SwarmSubtask::new("audit_auth", "Audit authentication and session management code for vulnerabilities"));
    let api  = dag.add_task(SwarmSubtask::new("audit_api",  "Scan API layer for injection, CORS misconfiguration, and rate-limit gaps"));
    let db   = dag.add_task(SwarmSubtask::new("audit_db",   "Review database access patterns for SQL injection and over-privileged queries"));
    let deps = dag.add_task(SwarmSubtask::new("audit_deps", "Check Cargo.lock for known CVEs in third-party dependencies"));
    let r    = dag.add_task(SwarmSubtask::new("security_report", "Compile all module findings into a prioritised security report"));

    dag.add_dependency(auth, r)?;
    dag.add_dependency(api, r)?;
    dag.add_dependency(db, r)?;
    dag.add_dependency(deps, r)?;

    let executor: SubtaskExecutorFn =
        Arc::new(move |_idx, task: SwarmSubtask| -> BoxFuture<'static, GlobalResult<String>> {
            let id = task.id.clone();
            let desc = task.description.clone();
            Box::pin(async move {
                let out = match id.as_str() {
                    "audit_auth" => "auth_audit: 1 HIGH (session token not rotated on privilege escalation), 2 MEDIUM (missing MFA enforcement on admin routes)".to_string(),
                    "audit_api"  => "api_audit: 0 HIGH, 1 MEDIUM (CORS wildcard on /internal/* routes), 3 LOW (missing rate-limit on password-reset endpoint)".to_string(),
                    "audit_db"   => "db_audit: 0 HIGH, 0 MEDIUM — all queries use parameterised statements. 1 INFO: db user has DROP TABLE privilege (unnecessary)".to_string(),
                    "audit_deps" => "deps_audit: 2 HIGH CVEs (RUSTSEC-2024-0031 in rustls 0.21.6, RUSTSEC-2024-0019 in h2 0.3.20) — upgrade required".to_string(),
                    "security_report" => {
                        let has_context = desc.contains("Map Phase Outputs");
                        format!(
                            "security_report_ok: {} — CRITICAL actions: upgrade rustls+h2, rotate session tokens on escalation. MEDIUM: fix CORS, enforce MFA. Full report: 3 HIGH, 4 MEDIUM, 3 LOW, 1 INFO.",
                            if has_context { "all 4 modules audited" } else { "no module data" }
                        )
                    }
                    _ => "done".to_string(),
                };
                Ok(out.to_string())
            })
        });

    let summary = MapReduceScheduler::new().execute(&mut dag, executor).await?;

    println!("Map phase (parallel module auditors):");
    for r in &summary.results {
        if r.task_id == "security_report" { continue; }
        let status = if r.outcome.is_success() { "OK" } else { "FAIL" };
        println!("  [{}] {} -> {}", status, r.task_id, r.outcome.output().unwrap_or("(no output)"));
    }

    if let Some(r) = summary.results.iter().find(|r| r.task_id == "security_report") {
        let status = if r.outcome.is_success() { "OK" } else { "FAIL" };
        println!("\nReduce phase (security compiler):\n  [{}] {} -> {}",
            status, r.task_id, r.outcome.output().unwrap_or("(no output)"));
    }

    println!("\nsucceeded={} failed={} total_time={:?}",
        summary.succeeded, summary.failed, summary.total_wall_time);

    Ok(())
}
