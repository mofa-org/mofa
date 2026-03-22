//! # Swarm HITL Gate — End-to-End Demo
//!
//! Demonstrates the full pipeline:
//!
//! ```text
//! analyze_offline_with_risk()          ← keyword risk classification
//!         │
//!         ▼  RiskAwareAnalysis
//! SwarmHITLGate::wrap_executor()      ← intercepts High/Critical tasks
//!         │                              with_intercept_when() for custom logic
//!         │                              HITLNotifier observer for side-effects
//!         ▼  gated executor
//! ParallelScheduler::execute()         ← runs the DAG concurrently
//!         │
//!         ▼  SchedulerSummary + HITLGateMetrics
//! enrich_summary() + print metrics
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
    HITLDecision, HITLGateMetrics, HITLMode, HITLNotifier, ParallelScheduler, SwarmHITLGate,
    SwarmScheduler, SwarmSchedulerConfig, SwarmSubtask, TaskAnalyzer,
};
use mofa_kernel::hitl::ReviewResponse;

// ── Demo notifier: prints every gate event to stdout ─────────────────────────

struct ConsoleNotifier;

impl HITLNotifier for ConsoleNotifier {
    fn on_intercepted(&self, task: &SwarmSubtask) {
        println!(
            "   [gate] intercepted '{}' (risk: {:?})",
            task.id, task.risk_level
        );
    }

    fn on_decision(&self, task: &SwarmSubtask, decision: HITLDecision, latency_ms: u64) {
        println!(
            "   [gate] decision for '{}': {:?} ({} ms)",
            task.id, decision, latency_ms
        );
    }
}

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

    let gate = Arc::new(
        SwarmHITLGate::new(Arc::clone(&manager), HITLMode::Optional, analysis.dag.id.clone())
            // Only intercept tasks that require human sign-off or touch payment.
            .with_intercept_when(|task| {
                task.hitl_required
                    || task.description.to_lowercase().contains("charge")
                    || task.description.to_lowercase().contains("pay")
            })
            // Attach the console notifier to observe every gate event.
            .with_notifier(Arc::new(ConsoleNotifier)),
    );

    let inner_executor: mofa_foundation::swarm::SubtaskExecutorFn =
        Arc::new(|_idx, task: SwarmSubtask| {
            Box::pin(async move {
                println!("   ⚙️  Executing: {}", task.description);
                Ok(format!("{}-done", task.id))
            })
        });

    // Clone the Arc so we can call gate.enrich_summary() after execution.
    let gated_executor = gate.clone().wrap_executor(inner_executor);

    println!("\n🚀 Starting parallel execution with HITLMode::Optional ...\n");

    let mut dag = analysis.dag;
    let cfg = SwarmSchedulerConfig::default();
    let summary = ParallelScheduler::with_config(cfg)
        .execute(&mut dag, gated_executor)
        .await?;

    // Attach HITL metrics to the summary.
    let summary = gate.enrich_summary(summary);

    // ── Phase 5: Print summary + gate metrics ──────────────────────────────

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

    if let Some(m) = &summary.hitl_stats {
        println!("\n════════════════════════ HITL Metrics ═════════════════════");
        println!("  Intercepted          : {}", m.intercepted);
        println!("  Approved             : {}", m.approved);
        println!("  Modified             : {}", m.modified);
        println!("  Rejected             : {}", m.rejected);
        println!("  Auto-approved (timeout): {}", m.auto_approved_timeout);
        println!("  Avg review latency   : {} ms", m.avg_review_latency_ms());
        println!("  Total review latency : {} ms", m.total_review_latency_ms);
    }

    println!("═══════════════════════════════════════════════════════════\n");

    Ok(())
}
