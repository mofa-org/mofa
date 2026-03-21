//! Parallel: Multi-Language Document Translation
//!
//! Scenario: Translate a product release note into EN, ES, FR, and DE
//! simultaneously. All four translations run in parallel, then results
//! are collected for publishing.
//!
//! Run: RUST_LOG=info cargo run -p swarm_coordination_patterns --bin parallel_translation_pipeline

use std::sync::Arc;

use anyhow::Result;
use futures::future::BoxFuture;
use mofa_foundation::swarm::{
    ParallelScheduler, SubtaskDAG, SubtaskExecutorFn, SwarmScheduler, SwarmSubtask,
};
use mofa_kernel::agent::types::error::GlobalResult;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    println!("=== Parallel: Multi-Language Document Translation ===");
    println!("DAG: translate_en, translate_es, translate_fr, translate_de (all parallel)\n");

    let mut dag = SubtaskDAG::new("translation");

    dag.add_task(SwarmSubtask::new("translate_en", "Translate release note to English (US)"));
    dag.add_task(SwarmSubtask::new("translate_es", "Translate release note to Spanish (LATAM)"));
    dag.add_task(SwarmSubtask::new("translate_fr", "Translate release note to French (EU)"));
    dag.add_task(SwarmSubtask::new("translate_de", "Translate release note to German (DE)"));

    let executor: SubtaskExecutorFn =
        Arc::new(move |_idx, task: SwarmSubtask| -> BoxFuture<'static, GlobalResult<String>> {
            let id = task.id.clone();
            Box::pin(async move {
                let out = match id.as_str() {
                    "translate_en" => "en_ok: 'MoFA v2.1 — Swarm coordination patterns now GA. See docs.mofa.dev/swarm.'",
                    "translate_es" => "es_ok: 'MoFA v2.1 — Patrones de coordinación de enjambre ahora disponibles. Ver docs.mofa.dev/swarm.'",
                    "translate_fr" => "fr_ok: 'MoFA v2.1 — Les patrons de coordination de swarm sont désormais GA. Voir docs.mofa.dev/swarm.'",
                    "translate_de" => "de_ok: 'MoFA v2.1 — Swarm-Koordinationsmuster jetzt allgemein verfügbar. Siehe docs.mofa.dev/swarm.'",
                    _              => "done",
                };
                Ok(out.to_string())
            })
        });

    let summary = ParallelScheduler::new().execute(&mut dag, executor).await?;

    println!("Translations (ran concurrently):");
    for r in &summary.results {
        let status = if r.outcome.is_success() { "OK" } else { "FAIL" };
        println!("  [{}] {} -> {}", status, r.task_id, r.outcome.output().unwrap_or("(no output)"));
    }

    println!("\nsucceeded={} failed={} total_time={:?}",
        summary.succeeded, summary.failed, summary.total_wall_time);

    Ok(())
}
