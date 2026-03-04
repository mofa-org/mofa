//! Memory-budgeted scheduler demo
//!
//! Demonstrates real-world admission control for a multi-model inference server.
//!
//! Scenario: A server with 24 GB GPU memory handles concurrent model-loading
//! requests. The scheduler prevents OOM by deferring or rejecting requests
//! that would exceed safe memory thresholds.
//!
//! Run: `cargo run -p memory_scheduler_demo`

use mofa_foundation::scheduler::{AdmissionOutcome, MemoryBudget, MemoryPolicy, MemoryScheduler};

fn main() {
    println!("=== Memory-Budgeted Scheduler Demo ===\n");

    // ── Setup: 24 GB GPU server ────────────────────────────────────
    let policy = MemoryPolicy::new(
        24_576, // 24 GB capacity
        0.75,   // defer above 75% (18 GB)
        0.90,   // reject above 90% (22 GB)
    );
    let budget = MemoryBudget::new(24_576);
    let mut scheduler = MemoryScheduler::new(policy, budget);

    println!("Server: 24 GB GPU | Defer >75% | Reject >90%\n");

    // ── Request 1: Load Llama-3-13B (~13 GB) ───────────────────────
    let decision = scheduler.evaluate(13_312);
    println!("Request 1: Llama-3-13B (13 GB)");
    println!("  Decision: {} — {}", decision.outcome, decision.reason);
    assert!(decision.is_accepted());
    scheduler.allocate(13_312);
    println!(
        "  Usage: {:.0}% ({} MB / 24576 MB)\n",
        scheduler.usage_percent(),
        scheduler.used_mb()
    );

    // ── Request 2: Load Mistral-7B (~7 GB) → Deferred ─────────────
    let decision = scheduler.evaluate(7_168);
    println!("Request 2: Mistral-7B (7 GB)");
    println!("  Decision: {} — {}", decision.outcome, decision.reason);
    assert!(decision.is_deferred());

    // Defer it into the fairness queue
    scheduler.defer("req-mistral-7b", 7_168);
    println!(
        "  Queued for retry (deferred count: {})\n",
        scheduler.deferred_count()
    );

    // ── Request 3: Load another 13B → Rejected ────────────────────
    let decision = scheduler.evaluate(13_312);
    println!("Request 3: Llama-3-13B #2 (13 GB)");
    println!("  Decision: {} — {}", decision.outcome, decision.reason);
    assert!(decision.is_rejected());
    println!();

    // ── Release Request 1 → Process deferred ──────────────────────
    println!("--- Llama-3-13B inference complete, releasing memory ---\n");
    scheduler.release(13_312);
    println!(
        "  Usage after release: {:.0}% ({} MB)\n",
        scheduler.usage_percent(),
        scheduler.used_mb()
    );

    // Now the deferred Mistral-7B can be processed
    if let Some(deferred) = scheduler.try_dequeue() {
        println!(
            "Processing deferred: {} ({} MB)",
            deferred.id, deferred.required_mb
        );
        scheduler.allocate(deferred.required_mb);
        println!(
            "  Usage: {:.0}% ({} MB)\n",
            scheduler.usage_percent(),
            scheduler.used_mb()
        );
    }

    // ── Stability control demo ────────────────────────────────────
    println!("--- Stability Control ---");
    println!("  Can switch profile? {}", scheduler.can_switch_profile());
    scheduler.record_switch();
    println!(
        "  Recorded switch. Can switch again? {}",
        scheduler.can_switch_profile()
    );
    println!("  (5-second cooldown prevents profile thrashing)\n");

    // ── Edge device scenario ──────────────────────────────────────
    println!("--- Edge Device: Raspberry Pi (4 GB) ---");
    let edge = MemoryScheduler::with_capacity(4_096);
    let decision = edge.evaluate(3_500);
    println!(
        "  Load 3.5 GB model: {} — {}",
        decision.outcome, decision.reason
    );

    let decision = edge.evaluate(2_000);
    println!(
        "  Load 2.0 GB model: {} — {}",
        decision.outcome, decision.reason
    );

    println!("\n=== Demo Complete ===");
}
