//! Planning Loop Integration Example
//!
//! Demonstrates the 4-phase planning loop (Plan → Execute → Reflect → Synthesize)
//! using the kernel planning types and the foundation `PlanningExecutor`.
//!
//! This example shows how multiple MoFA components work together:
//!
//! - **`mofa_kernel::workflow::planning`** — Core types (`Plan`, `PlanStep`,
//!   `Planner` trait, `PlanningConfig`, `PlanningEvent`)
//! - **`mofa_foundation::llm::planning_executor`** — `PlanningExecutor` engine
//!   with DAG-aware execution, reflection, and streaming events
//!
//! A mock planner and step executor are used so the example runs without
//! an LLM API key.  Replace `ResearchPlanner` with `LLMPlanner` and
//! `SimulatedStepExecutor` with a real `StepExecutor` for production use.
//!
//! # Scenarios demonstrated
//!
//! 1. **Linear execution** — steps with dependencies run in topological order
//! 2. **Parallel fan-out** — independent steps run in the same batch
//! 3. **Retry + reflection** — a step fails once, gets retried with feedback
//! 4. **Event streaming** — `run_with_channel()` provides real-time events
//!
//! # Usage
//!
//! ```bash
//! cargo run -p planning_loop
//! ```

use std::sync::Arc;

use async_trait::async_trait;
use tracing::{info, warn};

use mofa_kernel::agent::error::{AgentError, AgentResult};
use mofa_kernel::workflow::planning::{
    Plan, PlanStep, PlanStepOutput, Planner, PlanningConfig, PlanningEvent, ReflectionVerdict,
};

use mofa_foundation::llm::planning_executor::{PlanningExecutor, StepExecutor};

// ---------------------------------------------------------------------------
// Mock Planner — simulates an LLM decomposing a research task into steps
// ---------------------------------------------------------------------------

/// A mock planner that decomposes a goal into a research workflow DAG.
///
/// In production, replace this with `LLMPlanner` from `mofa_foundation::llm`
/// which calls an LLM to generate the plan, reflect on outputs, and synthesize.
struct ResearchPlanner;

#[async_trait]
impl Planner for ResearchPlanner {
    async fn decompose(&self, goal: &str) -> AgentResult<Plan> {
        info!("📋 Decomposing goal: {goal}");

        // Build a DAG-based plan:
        //
        //   search_papers ──┐
        //                   ├──▶ cross_reference ──▶ write_summary
        //   search_blogs  ──┘
        //
        // search_papers and search_blogs have no mutual dependency,
        // so the executor can batch them together (parallel fan-out).
        let plan = Plan::new(goal)
            .add_step(
                PlanStep::new("search_papers", "Search academic papers on the topic")
                    .with_tool("web_search")
                    .with_criterion("Returns at least 3 relevant papers"),
            )
            .add_step(
                PlanStep::new("search_blogs", "Search developer blogs for practical insights")
                    .with_tool("web_search")
                    .with_criterion("Returns at least 2 blog posts"),
            )
            .add_step(
                PlanStep::new(
                    "cross_reference",
                    "Cross-reference findings from papers and blogs",
                )
                .depends_on("search_papers")
                .depends_on("search_blogs")
                .with_criterion("Identifies common themes and contradictions")
                .with_max_retries(3),
            )
            .add_step(
                PlanStep::new(
                    "write_summary",
                    "Write a structured summary with citations",
                )
                .depends_on("cross_reference")
                .with_criterion("Summary is at least 200 words with references"),
            );

        Ok(plan)
    }

    async fn reflect(&self, step: &PlanStep, result: &str) -> AgentResult<ReflectionVerdict> {
        // Simulate reflection: the "cross_reference" step's first attempt is
        // deemed insufficient, triggering a retry with feedback.
        if step.id == "cross_reference" && step.attempts < 2 {
            info!("🔍 Reflection: cross_reference needs more depth");
            return Ok(ReflectionVerdict::Retry(
                "Analysis is too surface-level. Include quantitative comparisons.".into(),
            ));
        }

        // All other steps are accepted on first attempt
        info!(
            "✅ Reflection: step '{}' output accepted ({} chars)",
            step.id,
            result.len()
        );
        Ok(ReflectionVerdict::Accept)
    }

    async fn replan(
        &self,
        _plan: &Plan,
        failed_step: &PlanStep,
        error: &str,
    ) -> AgentResult<Plan> {
        warn!(
            "🔄 Replanning after failure in step '{}': {error}",
            failed_step.id
        );
        // Produce a simplified recovery plan
        Ok(Plan::new("Recovery plan")
            .add_step(PlanStep::new("fallback_search", "Broader search as fallback"))
            .add_step(
                PlanStep::new("write_summary", "Write summary from fallback")
                    .depends_on("fallback_search"),
            ))
    }

    async fn synthesize(&self, goal: &str, results: &[PlanStepOutput]) -> AgentResult<String> {
        info!("🧩 Synthesizing {} step results", results.len());

        let mut synthesis = format!("# Research Report: {goal}\n\n");

        for result in results {
            synthesis.push_str(&format!("## {}\n{}\n\n", result.step_id, result.output));
        }

        synthesis.push_str("---\n*Generated by MoFA Planning Loop*\n");

        Ok(synthesis)
    }
}

// ---------------------------------------------------------------------------
// Mock Step Executor — simulates executing each step
// ---------------------------------------------------------------------------

/// Simulates step execution with deterministic outputs.
///
/// In production, replace with `SimpleStepExecutor` (tool-based) or a custom
/// `StepExecutor` that invokes an LLM agent per step.
struct SimulatedStepExecutor;

#[async_trait]
impl StepExecutor for SimulatedStepExecutor {
    async fn execute_step(
        &self,
        step: &PlanStep,
        dependency_outputs: &[(String, String)],
        retry_feedback: Option<&str>,
    ) -> AgentResult<String> {
        // Log context from dependencies (shows DAG data flow)
        if !dependency_outputs.is_empty() {
            info!(
                "  ↳ Step '{}' has context from {} dependencies",
                step.id,
                dependency_outputs.len()
            );
        }

        if let Some(feedback) = retry_feedback {
            info!("  ↳ Step '{}' retrying with feedback: {feedback}", step.id);
        }

        // Simulate work with a small delay
        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

        // Return realistic mock outputs per step
        let output = match step.id.as_str() {
            "search_papers" => {
                "Found 4 papers:\n\
                 1. 'Async Runtime Design' (Smith 2024)\n\
                 2. 'Zero-Cost Futures in Rust' (Lee 2023)\n\
                 3. 'Structured Concurrency Patterns' (Brown 2024)\n\
                 4. 'Tokio Internals' (Matsakis 2023)"
                    .to_string()
            }
            "search_blogs" => {
                "Found 3 blog posts:\n\
                 1. 'Practical async/await patterns' — blog.rust-lang.org\n\
                 2. 'Understanding Pin and Unpin' — fasterthanli.me\n\
                 3. 'Async Rust Made Simple' — tokio.rs/blog"
                    .to_string()
            }
            "cross_reference" => {
                if retry_feedback.is_some() {
                    // Second attempt: richer analysis after reflection feedback
                    "Cross-reference (revised): All sources agree tokio is dominant. \
                     Papers cite 85% market share. Blogs emphasize ergonomics over \
                     performance. Key contradiction: papers favor structured concurrency \
                     while blogs prefer task::spawn patterns."
                        .to_string()
                } else {
                    // First attempt: shallow
                    "Cross-reference: Papers and blogs both discuss async.".to_string()
                }
            }
            "write_summary" => format!(
                "Async Rust research summary based on {} source(s). \
                 Key finding: structured concurrency is emerging as best practice, \
                 with tokio remaining the dominant runtime at 85% adoption.",
                dependency_outputs.len()
            ),
            other => format!("Output for step '{other}'"),
        };

        Ok(output)
    }
}

// ---------------------------------------------------------------------------
// Main — runs two scenarios
// ---------------------------------------------------------------------------

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_env_filter("info")
        .with_target(false)
        .init();

    info!("╔══════════════════════════════════════════════════════════╗");
    info!("║  MoFA Planning Loop — Integration Example               ║");
    info!("╚══════════════════════════════════════════════════════════╝");

    // ------------------------------------------------------------------
    // Scenario 1: Batch run — collect all events after completion
    // ------------------------------------------------------------------
    info!("");
    info!("━━━ Scenario 1: Batch execution with collected events ━━━");

    let planner = Arc::new(ResearchPlanner);
    let step_executor = Arc::new(SimulatedStepExecutor);
    let config = PlanningConfig::new()
        .with_max_replans(2)
        .with_max_parallel_steps(4)
        .with_step_timeout(5000);

    let executor = PlanningExecutor::new(
        Arc::clone(&planner),
        Arc::clone(&step_executor),
        config.clone(),
    );

    let (result, events) = executor.run("Research Rust async patterns").await?;

    info!("────── Event Log ──────");
    for event in &events {
        match event {
            PlanningEvent::PlanCreated { plan } => {
                info!("  📋 Plan created: '{}' ({} steps)", plan.goal, plan.steps.len());
                for step in &plan.steps {
                    let deps = if step.depends_on.is_empty() {
                        String::from("(root)")
                    } else {
                        format!("← {}", step.depends_on.join(", "))
                    };
                    info!("     • {} {}", step.id, deps);
                }
            }
            PlanningEvent::StepStarted { step_id, description } => {
                info!("  ▶ Started: {step_id} — {description}");
            }
            PlanningEvent::StepCompleted { step_id, result: res } => {
                let preview: String = res.chars().take(80).collect();
                info!("  ✅ Completed: {step_id} — {preview}…");
            }
            PlanningEvent::StepFailed { step_id, error, will_retry } => {
                info!("  ❌ Failed: {step_id} — {error} (retry={will_retry})");
            }
            PlanningEvent::StepRetry { step_id, attempt, feedback } => {
                info!("  🔄 Retry: {step_id} (attempt {attempt}) — {feedback}");
            }
            PlanningEvent::ReplanTriggered { iteration, reason } => {
                info!("  ⚠️  Replan #{iteration}: {reason}");
            }
            PlanningEvent::SynthesisStarted { num_results } => {
                info!("  🧩 Synthesis started ({num_results} results)");
            }
            PlanningEvent::PlanningComplete { final_answer } => {
                let preview: String = final_answer.chars().take(100).collect();
                info!("  🏁 Complete: {preview}…");
            }
            _ => {
                info!("  ? Unknown event");
            }
        }
    }

    info!("");
    info!("────── Final Output ──────");
    info!("{result}");

    // ------------------------------------------------------------------
    // Scenario 2: Streaming — consume events in real-time
    // ------------------------------------------------------------------
    info!("");
    info!("━━━ Scenario 2: Streaming execution with live events ━━━");

    let executor2 = PlanningExecutor::new(planner, step_executor, config);
    let (mut rx, handle) =
        executor2.run_with_channel("Investigate async state machines".to_string());

    // Consume events as they arrive
    let mut event_count = 0u32;
    while let Some(event) = rx.recv().await {
        event_count += 1;
        match &event {
            PlanningEvent::StepStarted { step_id, .. } => {
                info!("  [stream] ▶ {step_id}");
            }
            PlanningEvent::StepCompleted { step_id, .. } => {
                info!("  [stream] ✅ {step_id}");
            }
            PlanningEvent::StepRetry { step_id, attempt, .. } => {
                info!("  [stream] 🔄 {step_id} (attempt {attempt})");
            }
            PlanningEvent::PlanningComplete { .. } => {
                info!("  [stream] 🏁 Planning complete");
            }
            _ => {}
        }
    }

    let final_result = handle.await??;
    info!("  Streamed {event_count} events");
    info!(
        "  Final answer: {}…",
        final_result.chars().take(80).collect::<String>()
    );

    info!("");
    info!("Example complete.");
    Ok(())
}
