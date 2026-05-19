//! Supervision: Database Shard Migration with DBA Recovery
//!
//! Scenario: Three database shards are migrated in parallel. Shard B
//! hits a foreign key constraint violation and fails mid-migration.
//! The DBA supervisor always runs and issues a remediation plan.
//!
//! Run: RUST_LOG=info cargo run -p swarm_coordination_patterns --bin supervision_data_migration

use std::sync::Arc;

use anyhow::Result;
use futures::future::BoxFuture;
use mofa_foundation::swarm::{
    SubtaskDAG, SubtaskExecutorFn, SupervisionScheduler, SwarmScheduler, SwarmSubtask,
};
use mofa_kernel::agent::types::error::GlobalResult;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    println!("=== Supervision: Database Shard Migration ===");
    println!("Workers: migrate_shard_a (OK), migrate_shard_b (FK VIOLATION), migrate_shard_c (OK)");
    println!("DBA supervisor always runs and issues remediation plan\n");

    let mut dag = SubtaskDAG::new("db-migration");

    let w1 = dag.add_task(SwarmSubtask::new("migrate_shard_a", "Run ALTER TABLE migrations on shard_a (users 0-999k)"));
    let w2 = dag.add_task(SwarmSubtask::new("migrate_shard_b", "Run ALTER TABLE migrations on shard_b (users 1M-1.99M)"));
    let w3 = dag.add_task(SwarmSubtask::new("migrate_shard_c", "Run ALTER TABLE migrations on shard_c (users 2M-2.99M)"));
    let sv = dag.add_task(SwarmSubtask::new("dba_supervisor",  "Review all shard migration outcomes and issue a remediation or sign-off plan"));

    dag.add_dependency(w1, sv)?;
    dag.add_dependency(w2, sv)?;
    dag.add_dependency(w3, sv)?;

    let executor: SubtaskExecutorFn =
        Arc::new(move |_idx, task: SwarmSubtask| -> BoxFuture<'static, GlobalResult<String>> {
            let id = task.id.clone();
            let desc = task.description.clone();
            Box::pin(async move {
                match id.as_str() {
                    "migrate_shard_a" => Ok("shard_a_ok: 14 migrations applied, 0 errors, 1.02M rows affected, duration=4m12s, checksum_verified".into()),
                    "migrate_shard_b" => Err(mofa_kernel::agent::types::error::GlobalError::runtime(
                        "migration_failed: shard_b migration 0012_add_org_fk aborted — ERROR 1452: foreign key constraint violation on user_org_memberships (orphaned org_id=48291), rollback complete",
                    )),
                    "migrate_shard_c" => Ok("shard_c_ok: 14 migrations applied, 0 errors, 1.01M rows affected, duration=4m08s, checksum_verified".into()),
                    "dba_supervisor"  => {
                        let has_context = desc.contains("Worker Results");
                        Ok(format!(
                            "remediation_plan: {} — shard_a and shard_c complete (2/3 shards migrated). shard_b: run data-cleanup script to remove orphaned org_id=48291 row, then re-run migration 0012 in next maintenance window. Production reads on shard_b temporarily routed to replica. ETA resume: 2h.",
                            if has_context { "all shard outcomes reviewed." } else { "no shard data." }
                        ))
                    }
                    _ => Ok("done".into()),
                }
            })
        });

    let summary = SupervisionScheduler::new().execute(&mut dag, executor).await?;

    println!("Shard migrations:");
    for r in &summary.results {
        if r.task_id == "dba_supervisor" { continue; }
        let label = if r.outcome.is_success() { "SUCCESS" } else { "FAILED " };
        let detail = r.outcome.output().unwrap_or_else(|| match &r.outcome {
            mofa_foundation::swarm::TaskOutcome::Failure(e) => e.as_str(),
            _ => "",
        });
        println!("  [{}] {} -> {}", label, r.task_id, detail);
    }

    if let Some(r) = summary.results.iter().find(|r| r.task_id == "dba_supervisor") {
        println!("\nDBA supervisor (always runs):\n  [OK] {} -> {}",
            r.task_id, r.outcome.output().unwrap_or("(no output)"));
    }

    println!("\nsucceeded={} failed={} total_time={:?}",
        summary.succeeded, summary.failed, summary.total_wall_time);

    Ok(())
}
