//! # Swarm HITL Gate — End-to-End Demo
//!
//! Demonstrates the full pipeline:
//!
//! ```text
//! analyze_offline_with_risk()          ← keyword risk classification
//!         │
//!         ▼  RiskAwareAnalysis
//! SwarmHITLGate::wrap_executor()      ← intercepts High/Critical tasks
//!         │
//!         ▼  gated executor
//! ParallelScheduler::execute()         ← runs the DAG concurrently
//!         │
//!         ▼  SchedulerSummary
//! print results
//! ```
//!
//! A background task auto-approves all pending HITL reviews so the demo
//! runs without human interaction.
//!
//! Run with:
//! ```bash
//! cargo run -p swarm_hitl_gate
//! ```

use std::sync::Arc;
use std::time::Duration;

use mofa_foundation::hitl::manager::{ReviewManager, ReviewManagerConfig};
use mofa_foundation::hitl::notifier::ReviewNotifier;
use mofa_foundation::hitl::policy_engine::ReviewPolicyEngine;
use mofa_foundation::hitl::store::InMemoryReviewStore;
use mofa_foundation::swarm::{
    HITLMode, ParallelScheduler, SwarmHITLGate, SwarmScheduler, SwarmSchedulerConfig,
    SwarmSubtask, TaskAnalyzer,
};
use mofa_kernel::hitl::ReviewResponse;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    // ── Phase 1: Offline risk decomposition (no LLM key required) ─────────

    let task = "fetch customer records then charge payment card then send confirmation email";
    println!("\n═══════════════════════════════════════════════════════════");
    println!("  Task: {task}");
    println!("═══════════════════════════════════════════════════════════\n");

    let analysis = TaskAnalyzer::analyze_offline_with_risk(task);

    println!("📊 Risk Summary");
    println!("   Low:      {}", analysis.risk_summary.low);
    println!("   Medium:   {}", analysis.risk_summary.medium);
    println!("   High:     {}", analysis.risk_summary.high);
    println!("   Critical: {}", analysis.risk_summary.critical);

    println!("\n🔴 HITL-Required Tasks: {:?}", analysis.hitl_required_tasks);
    println!("🛤️  Critical Path:       {:?}", analysis.critical_path);
    println!(
        "⏱️  Critical Path Duration: {}s\n",
        analysis.critical_path_duration_secs
    );

    println!("📋 Subtask Details:");
    for (_, task) in analysis.dag.all_tasks() {
        println!(
            "   [{:?}] {} — risk: {:?}, hitl: {}, est: {}s",
            task.status,
            task.description,
            task.risk_level,
            task.hitl_required,
            task.estimated_duration_secs.unwrap_or(0),
        );
    }

    // ── Phase 2: Build ReviewManager with in-memory store ─────────────────

    let store = Arc::new(InMemoryReviewStore::new());
    let notifier = Arc::new(ReviewNotifier::default());
    let policy_engine = Arc::new(ReviewPolicyEngine::default());
    let manager = Arc::new(ReviewManager::new(
        store,
        notifier,
        policy_engine,
        None,
        ReviewManagerConfig::default(),
    ));

    // ── Phase 3: Background auto-approver ─────────────────────────────────

    let manager_bg = Arc::clone(&manager);
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(Duration::from_millis(100)).await;
            let pending = manager_bg.list_pending(None, None).await.unwrap_or_default();
            for review in pending {
                println!("\n✅ Auto-approving HITL review for task: '{}'", review.node_id.as_deref().unwrap_or("?"));
                manager_bg
                    .resolve_review(
                        &review.id,
                        ReviewResponse::Approved {
                            comment: Some("Auto-approved for demo".to_string()),
                        },
                        "demo-auto-approver".to_string(),
                    )
                    .await
                    .unwrap_or_else(|e| eprintln!("resolve_review error: {e}"));
            }
        }
    });

    // ── Phase 4: Execute with SwarmHITLGate ───────────────────────────────

    let gate = Arc::new(SwarmHITLGate::new(
        Arc::clone(&manager),
        HITLMode::Optional,
        analysis.dag.id.clone(),
    ));

    // Mock executor: just echo the task ID.
    let inner_executor: mofa_foundation::swarm::SubtaskExecutorFn =
        Arc::new(|_idx, task: SwarmSubtask| {
            Box::pin(async move {
                println!("   ⚙️  Executing: {}", task.description);
                Ok(format!("{}-done", task.id))
            })
        });

    let gated_executor = gate.wrap_executor(inner_executor);

    println!("\n🚀 Starting parallel execution with HITLMode::Optional ...\n");

    let mut dag = analysis.dag;
    let cfg = SwarmSchedulerConfig::default();
    let summary = ParallelScheduler::with_config(cfg)
        .execute(&mut dag, gated_executor)
        .await?;

    // ── Phase 5: Print summary ─────────────────────────────────────────────

    println!("\n════════════════════════ Summary ══════════════════════════");
    println!(
        "  ✅ Succeeded : {}/{}",
        summary.succeeded, summary.total_tasks
    );
    println!("  ❌ Failed    : {}", summary.failed);
    println!("  ⏭️  Skipped   : {}", summary.skipped);
    println!(
        "  ⏱️  Wall time : {:.2}s",
        summary.total_wall_time.as_secs_f64()
    );
    println!(
        "  📈 Success rate: {:.0}%",
        summary.success_rate() * 100.0
    );
    println!("═══════════════════════════════════════════════════════════\n");

    Ok(())
}
