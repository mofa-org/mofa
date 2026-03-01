//! Planning Executor — 4-phase goal decomposition loop
//!
//! Orchestrates the Plan → Execute → Reflect → Synthesize lifecycle:
//!
//! 1. **Plan** — Calls [`Planner::decompose`] to produce a DAG-based plan.
//! 2. **Execute** — Runs steps respecting DAG dependencies. Steps within a
//!    batch are executed **sequentially** (parallel via `JoinSet` is planned).
//!    Batch size is limited by [`PlanningConfig::max_parallel_steps`].
//! 3. **Reflect** — After each step, calls [`Planner::reflect`]. On `Retry`,
//!    re-executes with feedback. On `Replan`, revises the plan.
//! 4. **Synthesize** — Calls [`Planner::synthesize`] to merge step outputs.
//!
//! # Integration Points
//!
//! - Uses [`ToolExecutor`] for tool access during step execution.
//! - Emits [`PlanningEvent`]s for observability.
//! - Respects [`PlanningConfig`] for batch sizing, retry limits, and per-step
//!   timeouts.

use std::collections::HashSet;
use std::sync::Arc;

use mofa_kernel::agent::error::{AgentError, AgentResult};
use mofa_kernel::workflow::planning::{
    Plan, PlanStep, Planner, PlanningConfig, PlanningEvent, ReflectionVerdict, StepStatus,
};
use std::time::Duration;
use tokio::sync::mpsc;

use super::tool_executor::ToolExecutor;

// ---------------------------------------------------------------------------
// Step Executor (what runs a single step)
// ---------------------------------------------------------------------------

/// Trait for executing a single plan step.
///
/// The default implementation sends the step description (with dependency
/// context) to the tool executor. Custom implementations can override this
/// to use an LLM agent, a code interpreter, etc.
#[async_trait::async_trait]
pub trait StepExecutor: Send + Sync {
    /// Execute a single plan step and return its textual output.
    ///
    /// # Arguments
    /// * `step` — the step to execute
    /// * `dependency_outputs` — outputs from steps this step depends on
    /// * `retry_feedback` — if this is a retry, the feedback from reflection
    async fn execute_step(
        &self,
        step: &PlanStep,
        dependency_outputs: &[(String, String)],
        retry_feedback: Option<&str>,
    ) -> AgentResult<String>;
}

// ---------------------------------------------------------------------------
// Simple Step Executor (tool-based)
// ---------------------------------------------------------------------------

/// A simple step executor that formats the step context and calls the
/// tool executor's first available tool matching the step's `tools_needed`.
///
/// For production use, prefer an LLM-backed executor that can reason
/// about the step description and choose tools dynamically.
pub struct SimpleStepExecutor {
    tools: Arc<dyn ToolExecutor>,
}

impl SimpleStepExecutor {
    /// Create a new [`SimpleStepExecutor`] with the given tool executor.
    pub fn new(tools: Arc<dyn ToolExecutor>) -> Self {
        Self { tools }
    }
}

#[async_trait::async_trait]
impl StepExecutor for SimpleStepExecutor {
    async fn execute_step(
        &self,
        step: &PlanStep,
        dependency_outputs: &[(String, String)],
        retry_feedback: Option<&str>,
    ) -> AgentResult<String> {
        // Build a prompt incorporating the step description and dependency context
        let mut prompt = format!("Task: {}\n", step.description);

        if !dependency_outputs.is_empty() {
            prompt.push_str("\nContext from previous steps:\n");
            for (dep_id, output) in dependency_outputs {
                prompt.push_str(&format!("- [{}]: {}\n", dep_id, output));
            }
        }

        if let Some(feedback) = retry_feedback {
            prompt.push_str(&format!("\nPrevious attempt feedback: {}\n", feedback));
        }

        // If the step specifies tools, try to call the first one
        if let Some(tool_name) = step.tools_needed.first() {
            let result = self
                .tools
                .execute(tool_name, &serde_json::json!({"input": prompt}).to_string())
                .await
                .map_err(|e| AgentError::ExecutionFailed(format!("Tool '{}': {}", tool_name, e)))?;
            Ok(result)
        } else {
            // No tools needed — just return the description as a pass-through
            Ok(format!(
                "Step '{}' completed: {}",
                step.id, step.description
            ))
        }
    }
}

// ---------------------------------------------------------------------------
// Planning Executor
// ---------------------------------------------------------------------------

/// Orchestrates the Plan → Execute → Reflect → Synthesize loop.
///
/// # Example
///
/// ```rust,ignore
/// use mofa_foundation::llm::planning_executor::PlanningExecutor;
///
/// let executor = PlanningExecutor::new(planner, step_executor, config);
/// let (result, events) = executor.run("Research Rust async patterns").await?;
/// ```
pub struct PlanningExecutor {
    planner: Arc<dyn Planner>,
    step_executor: Arc<dyn StepExecutor>,
    config: PlanningConfig,
}

impl PlanningExecutor {
    /// Create a new planning executor.
    pub fn new(
        planner: Arc<dyn Planner>,
        step_executor: Arc<dyn StepExecutor>,
        config: PlanningConfig,
    ) -> Self {
        Self {
            planner,
            step_executor,
            config,
        }
    }

    /// Run the full planning loop and return `(final_answer, events)`.
    ///
    /// Events are collected into a `Vec` for simplicity. For streaming,
    /// use [`run_with_channel`].
    pub async fn run(&self, goal: &str) -> AgentResult<(String, Vec<PlanningEvent>)> {
        let (tx, mut rx) = mpsc::unbounded_channel();

        let result = self.run_inner(goal, tx).await;

        // Drain all events
        let mut events = Vec::new();
        rx.close();
        while let Some(event) = rx.recv().await {
            events.push(event);
        }

        result.map(|answer| (answer, events))
    }

    /// Run the full planning loop, streaming events via a channel.
    ///
    /// Returns an `UnboundedReceiver` that yields events in real-time
    /// as the planning loop progresses, plus a `JoinHandle` for the
    /// final result.
    pub fn run_with_channel(
        &self,
        goal: String,
    ) -> (
        mpsc::UnboundedReceiver<PlanningEvent>,
        tokio::task::JoinHandle<AgentResult<String>>,
    )
    where
        Self: 'static,
    {
        let (tx, rx) = mpsc::unbounded_channel();
        let planner = Arc::clone(&self.planner);
        let step_executor = Arc::clone(&self.step_executor);
        let config = self.config.clone();

        let handle = tokio::spawn(async move {
            let executor = PlanningExecutor {
                planner,
                step_executor,
                config,
            };
            executor.run_inner(&goal, tx).await
        });

        (rx, handle)
    }

    // -----------------------------------------------------------------------
    // Internal execution loop
    // -----------------------------------------------------------------------

    async fn run_inner(
        &self,
        goal: &str,
        tx: mpsc::UnboundedSender<PlanningEvent>,
    ) -> AgentResult<String> {
        // Phase 1: Decompose
        let mut plan = self.planner.decompose(goal).await?;
        plan.validate()?;

        let _ = tx.send(PlanningEvent::PlanCreated { plan: plan.clone() });

        // Replan loop
        let mut replan_count: u32 = 0;

        loop {
            // Phase 2 + 3: Execute & Reflect
            match self.execute_plan(&mut plan, &tx).await {
                Ok(()) => break, // All steps completed
                Err(replan_err) => {
                    replan_count += 1;
                    if replan_count > self.config.max_replans {
                        return Err(AgentError::ExecutionFailed(format!(
                            "Exceeded max replans ({}): {}",
                            self.config.max_replans, replan_err
                        )));
                    }

                    // Find the failed step
                    let failed_step = plan
                        .steps
                        .iter()
                        .find(|s| matches!(s.status, StepStatus::Failed(_)))
                        .cloned();

                    if let Some(failed) = failed_step {
                        let error_msg = match &failed.status {
                            StepStatus::Failed(msg) => msg.clone(),
                            _ => "Unknown error".to_string(),
                        };

                        let _ = tx.send(PlanningEvent::ReplanTriggered {
                            iteration: replan_count,
                            reason: error_msg.clone(),
                        });

                        // Snapshot completed results before replacing the plan
                        let old_completed_results: Vec<(String, String)> = plan
                            .steps
                            .iter()
                            .filter(|s| s.status.is_success())
                            .map(|s| (s.id.clone(), s.result.clone().unwrap_or_default()))
                            .collect();

                        plan = self.planner.replan(&plan, &failed, &error_msg).await?;
                        plan.validate()?;

                        // Carry forward completed step results from the old
                        // plan so they remain available for dependency context
                        // and final synthesis.
                        for old_step in &old_completed_results {
                            if let Some(new_step) = plan.get_step_mut(&old_step.0) {
                                new_step.status = StepStatus::Completed;
                                new_step.result = Some(old_step.1.clone());
                            }
                        }

                        let _ = tx.send(PlanningEvent::PlanCreated { plan: plan.clone() });
                    } else {
                        return Err(AgentError::ExecutionFailed(
                            "Replan triggered but no failed step found".into(),
                        ));
                    }
                }
            }
        }

        // Phase 4: Synthesize
        let results = plan.completed_results();
        let _ = tx.send(PlanningEvent::SynthesisStarted {
            num_results: results.len(),
        });

        let final_answer = self.planner.synthesize(goal, &results).await?;

        let _ = tx.send(PlanningEvent::PlanningComplete {
            final_answer: final_answer.clone(),
        });

        Ok(final_answer)
    }

    /// Execute all steps in a plan, respecting DAG dependencies.
    ///
    /// Returns `Ok(())` if all steps complete, or `Err` if a step fails
    /// after exhausting retries (signaling the outer loop to replan).
    async fn execute_plan(
        &self,
        plan: &mut Plan,
        tx: &mpsc::UnboundedSender<PlanningEvent>,
    ) -> Result<(), String> {
        let mut completed: HashSet<String> = HashSet::new();

        // Collect IDs of steps already completed (from a previous iteration)
        for step in &plan.steps {
            if step.status.is_success() {
                completed.insert(step.id.clone());
            }
        }

        loop {
            let ready_ids = plan.ready_steps(&completed);

            if ready_ids.is_empty() {
                // Either all done or stuck (should not happen after validation)
                if plan.is_complete() {
                    return Ok(());
                }
                // Check: are there pending steps with unmet dependencies?
                let has_pending = plan.steps.iter().any(|s| s.status == StepStatus::Pending);
                if has_pending {
                    return Err("Deadlock: pending steps with unsatisfiable dependencies".into());
                }
                return Ok(());
            }

            // Limit concurrency
            let batch: Vec<String> = ready_ids
                .into_iter()
                .take(self.config.max_parallel_steps)
                .collect();

            // Execute batch sequentially. (Parallel execution via JoinSet
            // is a planned follow-up — requires decoupling plan mutation.)
            for step_id in batch {
                self.execute_single_step(plan, &step_id, &completed, tx)
                    .await?;
                // Mark completed
                if let Some(step) = plan.get_step(&step_id) {
                    if step.status.is_success() {
                        completed.insert(step_id);
                    }
                }
            }
        }
    }

    /// Execute a single step with retry and reflection.
    ///
    /// `max_retries` on a step is the **total number of attempts** (including
    /// the first). Setting it to `1` means one attempt, no retries.
    /// When `PlanningConfig::step_timeout_ms > 0`, each execution attempt
    /// is wrapped in `tokio::time::timeout`.
    async fn execute_single_step(
        &self,
        plan: &mut Plan,
        step_id: &str,
        _completed: &HashSet<String>,
        tx: &mpsc::UnboundedSender<PlanningEvent>,
    ) -> Result<(), String> {
        // Gather dependency outputs for context
        let dep_outputs: Vec<(String, String)> = {
            let step = plan.get_step(step_id).ok_or("Step not found")?;
            step.depends_on
                .iter()
                .filter_map(|dep_id| {
                    plan.get_step(dep_id)
                        .and_then(|dep| dep.result.as_ref().map(|r| (dep_id.clone(), r.clone())))
                })
                .collect()
        };

        // Mark as running
        if let Some(step) = plan.get_step_mut(step_id) {
            step.status = StepStatus::Running;
            let _ = tx.send(PlanningEvent::StepStarted {
                step_id: step_id.to_string(),
                description: step.description.clone(),
            });
        }

        // Retry loop
        let max_retries = plan
            .get_step(step_id)
            .map(|s| s.max_retries)
            .unwrap_or(self.config.default_step_retries);

        let mut attempt: u32 = 0;
        let mut last_feedback: Option<String> = None;

        loop {
            attempt += 1;
            if let Some(step) = plan.get_step_mut(step_id) {
                step.attempts = attempt;
            }

            // Execute — pass retry feedback so the executor can improve.
            // Apply per-step timeout if configured.
            let step_snapshot = plan.get_step(step_id).ok_or("Step not found")?.clone();
            let exec_result = if self.config.step_timeout_ms > 0 {
                let timeout_dur = Duration::from_millis(self.config.step_timeout_ms);
                match tokio::time::timeout(
                    timeout_dur,
                    self.step_executor.execute_step(
                        &step_snapshot,
                        &dep_outputs,
                        last_feedback.as_deref(),
                    ),
                )
                .await
                {
                    Ok(result) => result,
                    Err(_elapsed) => Err(AgentError::ExecutionFailed(format!(
                        "Step '{}' timed out after {}ms",
                        step_id, self.config.step_timeout_ms
                    ))),
                }
            } else {
                self.step_executor
                    .execute_step(&step_snapshot, &dep_outputs, last_feedback.as_deref())
                    .await
            };
            match exec_result {
                Ok(output) => {
                    // Reflect
                    let step_snap = plan.get_step(step_id).ok_or("Step not found")?.clone();
                    let verdict = self
                        .planner
                        .reflect(&step_snap, &output)
                        .await
                        .map_err(|e| e.to_string())?;

                    match verdict {
                        ReflectionVerdict::Accept => {
                            if let Some(step) = plan.get_step_mut(step_id) {
                                step.status = StepStatus::Completed;
                                step.result = Some(output.clone());
                            }
                            let _ = tx.send(PlanningEvent::StepCompleted {
                                step_id: step_id.to_string(),
                                result: output,
                            });
                            return Ok(());
                        }
                        ReflectionVerdict::Retry(feedback) => {
                            if attempt >= max_retries {
                                // Exhausted retries — mark failed and trigger replan
                                let err_msg =
                                    format!("Exhausted {} retries: {}", max_retries, feedback);
                                if let Some(step) = plan.get_step_mut(step_id) {
                                    step.status = StepStatus::Failed(err_msg.clone());
                                }
                                let _ = tx.send(PlanningEvent::StepFailed {
                                    step_id: step_id.to_string(),
                                    error: err_msg.clone(),
                                    will_retry: false,
                                });
                                return Err(err_msg);
                            }
                            let _ = tx.send(PlanningEvent::StepRetry {
                                step_id: step_id.to_string(),
                                attempt,
                                feedback: feedback.clone(),
                            });
                            last_feedback = Some(feedback);
                            continue; // Retry
                        }
                        ReflectionVerdict::Replan(reason) => {
                            let err_msg = format!("Replan requested: {}", reason);
                            if let Some(step) = plan.get_step_mut(step_id) {
                                step.status = StepStatus::Failed(err_msg.clone());
                            }
                            let _ = tx.send(PlanningEvent::StepFailed {
                                step_id: step_id.to_string(),
                                error: err_msg.clone(),
                                will_retry: false,
                            });
                            return Err(err_msg);
                        }
                        _ => {
                            // Future verdict variants — treat as accept
                            if let Some(step) = plan.get_step_mut(step_id) {
                                step.status = StepStatus::Completed;
                                step.result = Some(output.clone());
                            }
                            let _ = tx.send(PlanningEvent::StepCompleted {
                                step_id: step_id.to_string(),
                                result: output,
                            });
                            return Ok(());
                        }
                    }
                }
                Err(exec_err) => {
                    let err_msg = exec_err.to_string();
                    if attempt >= max_retries {
                        if let Some(step) = plan.get_step_mut(step_id) {
                            step.status = StepStatus::Failed(err_msg.clone());
                        }
                        let _ = tx.send(PlanningEvent::StepFailed {
                            step_id: step_id.to_string(),
                            error: err_msg.clone(),
                            will_retry: false,
                        });
                        return Err(err_msg);
                    }
                    let _ = tx.send(PlanningEvent::StepFailed {
                        step_id: step_id.to_string(),
                        error: err_msg.clone(),
                        will_retry: true,
                    });
                    let _ = tx.send(PlanningEvent::StepRetry {
                        step_id: step_id.to_string(),
                        attempt,
                        feedback: format!("Execution error: {}", err_msg),
                    });
                    last_feedback = Some(err_msg);
                    continue; // Retry
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use mofa_kernel::workflow::planning::{PlanStep, PlanStepOutput};

    // -- Mock Planner ---------------------------------------------------

    struct MockPlanner {
        accept_all: bool,
    }

    impl MockPlanner {
        fn accepting() -> Self {
            Self { accept_all: true }
        }

        fn rejecting_then_replan() -> Self {
            Self { accept_all: false }
        }
    }

    #[async_trait::async_trait]
    impl Planner for MockPlanner {
        async fn decompose(&self, goal: &str) -> AgentResult<Plan> {
            Ok(Plan::new(goal)
                .add_step(PlanStep::new("step_1", "First step").with_max_retries(2))
                .add_step(
                    PlanStep::new("step_2", "Second step")
                        .depends_on("step_1")
                        .with_max_retries(2),
                ))
        }

        async fn reflect(&self, _step: &PlanStep, _result: &str) -> AgentResult<ReflectionVerdict> {
            if self.accept_all {
                Ok(ReflectionVerdict::Accept)
            } else {
                Ok(ReflectionVerdict::Replan("Mock replan".into()))
            }
        }

        async fn replan(
            &self,
            _plan: &Plan,
            _failed_step: &PlanStep,
            _error: &str,
        ) -> AgentResult<Plan> {
            // Return a simple single-step plan
            Ok(Plan::new("replanned goal")
                .add_step(PlanStep::new("recovery", "Recovery step").with_max_retries(2)))
        }

        async fn synthesize(&self, goal: &str, results: &[PlanStepOutput]) -> AgentResult<String> {
            let summary: Vec<String> = results
                .iter()
                .map(|r| format!("[{}] {}", r.step_id, r.output))
                .collect();
            Ok(format!("Synthesis for '{}': {}", goal, summary.join("; ")))
        }
    }

    // -- Mock Step Executor -----------------------------------------------

    struct MockStepExecutor;

    #[async_trait::async_trait]
    impl StepExecutor for MockStepExecutor {
        async fn execute_step(
            &self,
            step: &PlanStep,
            _dependency_outputs: &[(String, String)],
            _retry_feedback: Option<&str>,
        ) -> AgentResult<String> {
            Ok(format!("Output of {}", step.id))
        }
    }

    struct FailingStepExecutor {
        fail_step_id: String,
    }

    #[async_trait::async_trait]
    impl StepExecutor for FailingStepExecutor {
        async fn execute_step(
            &self,
            step: &PlanStep,
            _dependency_outputs: &[(String, String)],
            _retry_feedback: Option<&str>,
        ) -> AgentResult<String> {
            if step.id == self.fail_step_id {
                Err(AgentError::ExecutionFailed(format!(
                    "Simulated failure for {}",
                    step.id
                )))
            } else {
                Ok(format!("Output of {}", step.id))
            }
        }
    }

    // -- Tests ----------------------------------------------------------

    #[tokio::test]
    async fn test_linear_plan_execution() {
        let planner = Arc::new(MockPlanner::accepting());
        let executor = Arc::new(MockStepExecutor);
        let config = PlanningConfig::default();

        let planning_executor = PlanningExecutor::new(planner, executor, config);
        let (result, events) = planning_executor.run("Test goal").await.unwrap();

        assert!(result.contains("Synthesis"));
        assert!(result.contains("step_1"));
        assert!(result.contains("step_2"));

        // Verify event sequence
        let event_types: Vec<String> = events
            .iter()
            .map(|e| match e {
                PlanningEvent::PlanCreated { .. } => "PlanCreated".into(),
                PlanningEvent::StepStarted { .. } => "StepStarted".into(),
                PlanningEvent::StepCompleted { .. } => "StepCompleted".into(),
                PlanningEvent::SynthesisStarted { .. } => "SynthesisStarted".into(),
                PlanningEvent::PlanningComplete { .. } => "PlanningComplete".into(),
                _ => "Other".into(),
            })
            .collect();

        assert_eq!(event_types[0], "PlanCreated");
        assert!(event_types.contains(&"StepStarted".to_string()));
        assert!(event_types.contains(&"StepCompleted".to_string()));
        assert!(event_types.contains(&"SynthesisStarted".to_string()));
        assert_eq!(event_types.last().unwrap(), "PlanningComplete");
    }

    #[tokio::test]
    async fn test_step_failure_triggers_replan() {
        // Use a planner that accepts all reflections BUT the step executor fails
        struct ReplanAcceptingPlanner;

        #[async_trait::async_trait]
        impl Planner for ReplanAcceptingPlanner {
            async fn decompose(&self, _goal: &str) -> AgentResult<Plan> {
                Ok(Plan::new("goal")
                    .add_step(PlanStep::new("will_fail", "Fail step").with_max_retries(1)))
            }

            async fn reflect(
                &self,
                _step: &PlanStep,
                _result: &str,
            ) -> AgentResult<ReflectionVerdict> {
                Ok(ReflectionVerdict::Accept)
            }

            async fn replan(
                &self,
                _plan: &Plan,
                _failed_step: &PlanStep,
                _error: &str,
            ) -> AgentResult<Plan> {
                Ok(Plan::new("replanned")
                    .add_step(PlanStep::new("recovery", "Recovery").with_max_retries(2)))
            }

            async fn synthesize(
                &self,
                _goal: &str,
                results: &[PlanStepOutput],
            ) -> AgentResult<String> {
                Ok(format!("Recovered: {}", results.len()))
            }
        }

        let planner = Arc::new(ReplanAcceptingPlanner);
        let executor = Arc::new(FailingStepExecutor {
            fail_step_id: "will_fail".to_string(),
        });
        let config = PlanningConfig::new().with_max_replans(2);

        let planning_executor = PlanningExecutor::new(planner, executor, config);
        let (result, events) = planning_executor.run("Test replan").await.unwrap();

        assert!(result.contains("Recovered"));

        let has_replan = events
            .iter()
            .any(|e| matches!(e, PlanningEvent::ReplanTriggered { .. }));
        assert!(has_replan, "Expected a ReplanTriggered event");
    }

    #[tokio::test]
    async fn test_max_replans_exceeded() {
        // Planner always triggers replan, executor always fails
        struct AlwaysReplanPlanner;

        #[async_trait::async_trait]
        impl Planner for AlwaysReplanPlanner {
            async fn decompose(&self, _goal: &str) -> AgentResult<Plan> {
                Ok(Plan::new("goal")
                    .add_step(PlanStep::new("doomed", "Always fails").with_max_retries(1)))
            }

            async fn reflect(
                &self,
                _step: &PlanStep,
                _result: &str,
            ) -> AgentResult<ReflectionVerdict> {
                Ok(ReflectionVerdict::Accept)
            }

            async fn replan(
                &self,
                _plan: &Plan,
                _failed_step: &PlanStep,
                _error: &str,
            ) -> AgentResult<Plan> {
                Ok(Plan::new("replanned")
                    .add_step(PlanStep::new("doomed", "Still fails").with_max_retries(1)))
            }

            async fn synthesize(
                &self,
                _goal: &str,
                _results: &[PlanStepOutput],
            ) -> AgentResult<String> {
                Ok("never reached".into())
            }
        }

        let planner = Arc::new(AlwaysReplanPlanner);
        let executor = Arc::new(FailingStepExecutor {
            fail_step_id: "doomed".to_string(),
        });
        let config = PlanningConfig::new().with_max_replans(2);

        let planning_executor = PlanningExecutor::new(planner, executor, config);
        let result = planning_executor.run("Doomed goal").await;

        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("Exceeded max replans"),
            "Expected max replans error, got: {}",
            err
        );
    }

    #[tokio::test]
    async fn test_parallel_independent_steps() {
        struct ParallelPlanner;

        #[async_trait::async_trait]
        impl Planner for ParallelPlanner {
            async fn decompose(&self, _goal: &str) -> AgentResult<Plan> {
                Ok(Plan::new("goal")
                    .add_step(PlanStep::new("a", "Step A"))
                    .add_step(PlanStep::new("b", "Step B"))
                    .add_step(PlanStep::new("c", "Step C"))
                    .add_step(
                        PlanStep::new("join", "Join")
                            .depends_on("a")
                            .depends_on("b")
                            .depends_on("c"),
                    ))
            }

            async fn reflect(
                &self,
                _step: &PlanStep,
                _result: &str,
            ) -> AgentResult<ReflectionVerdict> {
                Ok(ReflectionVerdict::Accept)
            }

            async fn replan(
                &self,
                _plan: &Plan,
                _failed_step: &PlanStep,
                _error: &str,
            ) -> AgentResult<Plan> {
                unreachable!()
            }

            async fn synthesize(
                &self,
                _goal: &str,
                results: &[PlanStepOutput],
            ) -> AgentResult<String> {
                Ok(format!("Joined {} results", results.len()))
            }
        }

        let planner = Arc::new(ParallelPlanner);
        let executor = Arc::new(MockStepExecutor);
        let config = PlanningConfig::new().with_max_parallel_steps(3);

        let planning_executor = PlanningExecutor::new(planner, executor, config);
        let (result, events) = planning_executor.run("Parallel test").await.unwrap();

        assert!(result.contains("Joined 4 results"));

        // Verify all 4 steps completed
        let completed_count = events
            .iter()
            .filter(|e| matches!(e, PlanningEvent::StepCompleted { .. }))
            .count();
        assert_eq!(completed_count, 4);
    }

    #[tokio::test]
    async fn test_reflection_retry_then_accept() {
        use std::sync::atomic::{AtomicU32, Ordering};

        struct RetryThenAcceptPlanner {
            call_count: AtomicU32,
        }

        #[async_trait::async_trait]
        impl Planner for RetryThenAcceptPlanner {
            async fn decompose(&self, _goal: &str) -> AgentResult<Plan> {
                Ok(Plan::new("goal").add_step(PlanStep::new("step", "A step").with_max_retries(3)))
            }

            async fn reflect(
                &self,
                _step: &PlanStep,
                _result: &str,
            ) -> AgentResult<ReflectionVerdict> {
                let count = self.call_count.fetch_add(1, Ordering::SeqCst);
                if count < 2 {
                    Ok(ReflectionVerdict::Retry("Not good enough".into()))
                } else {
                    Ok(ReflectionVerdict::Accept)
                }
            }

            async fn replan(
                &self,
                _plan: &Plan,
                _failed_step: &PlanStep,
                _error: &str,
            ) -> AgentResult<Plan> {
                unreachable!()
            }

            async fn synthesize(
                &self,
                _goal: &str,
                results: &[PlanStepOutput],
            ) -> AgentResult<String> {
                Ok(format!("Done: {}", results[0].output))
            }
        }

        let planner = Arc::new(RetryThenAcceptPlanner {
            call_count: AtomicU32::new(0),
        });
        let executor = Arc::new(MockStepExecutor);
        let config = PlanningConfig::default();

        let planning_executor = PlanningExecutor::new(planner, executor, config);
        let (result, events) = planning_executor.run("Retry test").await.unwrap();

        assert!(result.contains("Done"));

        // Should have retry events
        let retry_count = events
            .iter()
            .filter(|e| matches!(e, PlanningEvent::StepRetry { .. }))
            .count();
        assert_eq!(retry_count, 2, "Expected 2 retries before accept");
    }

    #[tokio::test]
    async fn test_dependency_context_passed() {
        use std::sync::{Arc as StdArc, Mutex};

        #[derive(Clone)]
        struct CapturingExecutor {
            captured_deps: StdArc<Mutex<Vec<Vec<(String, String)>>>>,
        }

        #[async_trait::async_trait]
        impl StepExecutor for CapturingExecutor {
            async fn execute_step(
                &self,
                step: &PlanStep,
                dependency_outputs: &[(String, String)],
                _retry_feedback: Option<&str>,
            ) -> AgentResult<String> {
                self.captured_deps
                    .lock()
                    .unwrap()
                    .push(dependency_outputs.to_vec());
                Ok(format!("Result of {}", step.id))
            }
        }

        let planner = Arc::new(MockPlanner::accepting());
        let captured = StdArc::new(Mutex::new(Vec::new()));
        let executor = Arc::new(CapturingExecutor {
            captured_deps: captured.clone(),
        });
        let config = PlanningConfig::default();

        let planning_executor = PlanningExecutor::new(planner, executor, config);
        let _ = planning_executor.run("Dep test").await.unwrap();

        let deps = captured.lock().unwrap();
        // step_1 has no deps
        assert!(deps[0].is_empty());
        // step_2 depends on step_1
        assert_eq!(deps[1].len(), 1);
        assert_eq!(deps[1][0].0, "step_1");
        assert!(deps[1][0].1.contains("Result of step_1"));
    }
}
