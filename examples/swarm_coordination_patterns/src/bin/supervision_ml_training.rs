//! Supervision: ML Training with OOM Recovery
//!
//! Scenario: Three model trainers run in parallel with different
//! hyperparameter configs. The large-batch trainer runs OOM and fails.
//! The supervisor always runs and selects the best result from survivors.
//!
//! Run: RUST_LOG=info cargo run -p swarm_coordination_patterns --bin supervision_ml_training

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

    println!("=== Supervision: ML Training with OOM Recovery ===");
    println!("Workers: trainer_small (OK), trainer_large (OOM FAIL), trainer_medium (OK)");
    println!("Supervisor picks best surviving model\n");

    let mut dag = SubtaskDAG::new("ml-training");

    let w1 = dag.add_task(SwarmSubtask::new("trainer_small",  "Train with batch_size=32, lr=1e-4, epochs=10"));
    let w2 = dag.add_task(SwarmSubtask::new("trainer_large",  "Train with batch_size=512, lr=5e-4, epochs=10"));
    let w3 = dag.add_task(SwarmSubtask::new("trainer_medium", "Train with batch_size=128, lr=2e-4, epochs=10"));
    let sv = dag.add_task(SwarmSubtask::new("model_supervisor", "Review all training outcomes and select the best model for deployment"));

    dag.add_dependency(w1, sv)?;
    dag.add_dependency(w2, sv)?;
    dag.add_dependency(w3, sv)?;

    let executor: SubtaskExecutorFn =
        Arc::new(move |_idx, task: SwarmSubtask| -> BoxFuture<'static, GlobalResult<String>> {
            let id = task.id.clone();
            let desc = task.description.clone();
            Box::pin(async move {
                match id.as_str() {
                    "trainer_small"  => Ok("small_complete: val_loss=0.2341, val_acc=91.2%, checkpoint=ckpt_small_ep10.pt, gpu_mem_peak=4.1GB".into()),
                    "trainer_large"  => Err(mofa_kernel::agent::types::error::GlobalError::runtime(
                        "oom_error: CUDA out of memory at epoch 3 — batch_size=512 exceeds 16GB VRAM, checkpoint not saved",
                    )),
                    "trainer_medium" => Ok("medium_complete: val_loss=0.1987, val_acc=93.6%, checkpoint=ckpt_medium_ep10.pt, gpu_mem_peak=9.8GB".into()),
                    "model_supervisor" => {
                        let has_context = desc.contains("Worker Results");
                        Ok(format!(
                            "selection: DEPLOY_MEDIUM — {} trainer_medium wins (val_acc=93.6% vs 91.2%, trainer_large OOM). Promote ckpt_medium_ep10.pt to production. Recommend: retry trainer_large with gradient checkpointing to reduce VRAM.",
                            if has_context { "all training outcomes reviewed." } else { "no training data." }
                        ))
                    }
                    _ => Ok("done".into()),
                }
            })
        });

    let summary = SupervisionScheduler::new().execute(&mut dag, executor).await?;

    println!("Trainers:");
    for r in &summary.results {
        if r.task_id == "model_supervisor" { continue; }
        let label = if r.outcome.is_success() { "SUCCESS" } else { "FAILED " };
        let detail = r.outcome.output().unwrap_or_else(|| match &r.outcome {
            mofa_foundation::swarm::TaskOutcome::Failure(e) => e.as_str(),
            _ => "",
        });
        println!("  [{}] {} -> {}", label, r.task_id, detail);
    }

    if let Some(r) = summary.results.iter().find(|r| r.task_id == "model_supervisor") {
        println!("\nModel supervisor (always runs):\n  [OK] {} -> {}",
            r.task_id, r.outcome.output().unwrap_or("(no output)"));
    }

    println!("\nsucceeded={} failed={} total_time={:?}",
        summary.succeeded, summary.failed, summary.total_wall_time);

    Ok(())
}
