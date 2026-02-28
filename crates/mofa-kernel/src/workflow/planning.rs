//! Multi-Step Goal Decomposition & Planning
//!
//! Provides a structured planning loop for autonomous agents that separates
//! goal execution into four distinct phases:
//!
//! 1. **Plan** — Decompose a high-level goal into executable steps with DAG dependencies
//! 2. **Execute** — Run steps in topological order, parallelizing independent steps
//! 3. **Reflect** — Evaluate step outputs against completion criteria
//! 4. **Synthesize** — Combine all step results into a coherent final answer
//!
//! # Architecture
//!
//! This module defines the core types and traits (kernel layer).
//! The concrete `PlanningExecutor` and `LLMPlanner` implementations
//! are provided in `mofa-foundation`.
//!
//! # Example
//!
//! ```rust,ignore
//! use mofa_kernel::workflow::planning::{Plan, PlanStep, Planner, PlanningConfig};
//!
//! // Create a plan
//! let plan = Plan::new("Research Rust async patterns")
//!     .add_step(PlanStep::new("gather", "Search for blog posts on async/await")
//!         .with_tool("web_search"))
//!     .add_step(PlanStep::new("analyze", "Analyze the search results")
//!         .depends_on("gather"))
//!     .add_step(PlanStep::new("summarize", "Write a structured summary")
//!         .depends_on("analyze"));
//! ```

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

use crate::agent::error::{AgentError, AgentResult};

// ---------------------------------------------------------------------------
// Step Status
// ---------------------------------------------------------------------------

/// Status of a single plan step during execution.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub enum StepStatus {
    /// Waiting to be executed.
    #[default]
    Pending,
    /// Currently executing.
    Running,
    /// Finished successfully.
    Completed,
    /// Failed with an error message.
    Failed(String),
    /// Skipped (e.g. dependency failed and replan chose to skip).
    Skipped,
}

impl StepStatus {
    /// Returns `true` if the step has reached a terminal state.
    pub fn is_terminal(&self) -> bool {
        matches!(self, Self::Completed | Self::Failed(_) | Self::Skipped)
    }

    /// Returns `true` if the step completed successfully.
    pub fn is_success(&self) -> bool {
        matches!(self, Self::Completed)
    }
}

// ---------------------------------------------------------------------------
// Plan Step
// ---------------------------------------------------------------------------

/// A single executable step within a plan.
///
/// Steps form a DAG via `depends_on` edges. Independent steps (with no
/// shared dependencies) can be executed in parallel by the planning executor.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanStep {
    /// Unique identifier for this step within the plan.
    pub id: String,

    /// Human-readable description of what this step accomplishes.
    pub description: String,

    /// Tool names this step may invoke during execution.
    pub tools_needed: Vec<String>,

    /// IDs of steps that must complete before this step can start.
    ///
    /// Forms the edges of the dependency DAG.
    pub depends_on: Vec<String>,

    /// Criterion the planner uses to decide whether the step output is acceptable.
    pub completion_criterion: String,

    /// Maximum number of retry attempts before escalating to replan.
    pub max_retries: u32,

    /// Current execution status.
    #[serde(default)]
    pub status: StepStatus,

    /// Output produced by this step (populated after successful execution).
    pub result: Option<String>,

    /// Number of retries attempted so far.
    #[serde(default)]
    pub attempts: u32,
}

impl PlanStep {
    /// Create a new plan step with the given ID and description.
    pub fn new(id: impl Into<String>, description: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            description: description.into(),
            tools_needed: Vec::new(),
            depends_on: Vec::new(),
            completion_criterion: String::new(),
            max_retries: 2,
            status: StepStatus::Pending,
            result: None,
            attempts: 0,
        }
    }

    /// Add a tool dependency.
    pub fn with_tool(mut self, tool: impl Into<String>) -> Self {
        self.tools_needed.push(tool.into());
        self
    }

    /// Add a dependency on another step.
    pub fn depends_on(mut self, step_id: impl Into<String>) -> Self {
        self.depends_on.push(step_id.into());
        self
    }

    /// Set the completion criterion.
    pub fn with_criterion(mut self, criterion: impl Into<String>) -> Self {
        self.completion_criterion = criterion.into();
        self
    }

    /// Set maximum retry attempts.
    pub fn with_max_retries(mut self, max: u32) -> Self {
        self.max_retries = max;
        self
    }

    /// Returns `true` if all listed dependencies are met by `completed_ids`.
    pub fn dependencies_met(&self, completed_ids: &HashSet<String>) -> bool {
        self.depends_on
            .iter()
            .all(|dep| completed_ids.contains(dep))
    }

    /// Returns `true` if the step can still be retried.
    pub fn can_retry(&self) -> bool {
        self.attempts < self.max_retries
    }
}

// ---------------------------------------------------------------------------
// Plan
// ---------------------------------------------------------------------------

/// A structured execution plan produced by a [`Planner`].
///
/// Contains an ordered list of [`PlanStep`]s whose `depends_on` fields
/// describe a dependency DAG. The planning executor uses topological
/// ordering to determine execution sequence and parallelism.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Plan {
    /// The high-level goal this plan aims to achieve.
    pub goal: String,

    /// Ordered list of steps (insertion order is a hint, DAG is authoritative).
    pub steps: Vec<PlanStep>,

    /// ISO-8601 timestamp of when this plan was created.
    pub created_at: String,

    /// How many times this plan has been revised (0 = original plan).
    pub iteration: u32,
}

impl Plan {
    /// Create a new plan for the given goal.
    pub fn new(goal: impl Into<String>) -> Self {
        Self {
            goal: goal.into(),
            steps: Vec::new(),
            created_at: String::new(),
            iteration: 0,
        }
    }

    /// Add a step to the plan (builder pattern).
    pub fn add_step(mut self, step: PlanStep) -> Self {
        self.steps.push(step);
        self
    }

    /// Look up a step by its ID.
    pub fn get_step(&self, id: &str) -> Option<&PlanStep> {
        self.steps.iter().find(|s| s.id == id)
    }

    /// Get a mutable reference to a step by its ID.
    pub fn get_step_mut(&mut self, id: &str) -> Option<&mut PlanStep> {
        self.steps.iter_mut().find(|s| s.id == id)
    }

    /// Return the IDs of all steps that are ready to execute:
    /// status is `Pending` and all dependencies are completed.
    pub fn ready_steps(&self, completed: &HashSet<String>) -> Vec<String> {
        self.steps
            .iter()
            .filter(|s| s.status == StepStatus::Pending && s.dependencies_met(completed))
            .map(|s| s.id.clone())
            .collect()
    }

    /// Return `true` when every step has reached a terminal state.
    pub fn is_complete(&self) -> bool {
        self.steps.iter().all(|s| s.status.is_terminal())
    }

    /// Collect all completed step results as `(step_id, output)` pairs.
    pub fn completed_results(&self) -> Vec<StepResult> {
        self.steps
            .iter()
            .filter(|s| s.status.is_success())
            .map(|s| StepResult {
                step_id: s.id.clone(),
                output: s.result.clone().unwrap_or_default(),
            })
            .collect()
    }

    // -----------------------------------------------------------------------
    // DAG Validation
    // -----------------------------------------------------------------------

    /// Validate the plan's dependency DAG.
    ///
    /// Checks for:
    /// 1. References to non-existent step IDs
    /// 2. Duplicate step IDs
    /// 3. Cycles in the dependency graph
    pub fn validate(&self) -> AgentResult<()> {
        let ids: HashSet<&str> = self.steps.iter().map(|s| s.id.as_str()).collect();

        // Check for duplicate IDs
        if ids.len() != self.steps.len() {
            return Err(AgentError::ValidationFailed(
                "Plan contains duplicate step IDs".into(),
            ));
        }

        // Check for dangling dependency references
        for step in &self.steps {
            for dep in &step.depends_on {
                if !ids.contains(dep.as_str()) {
                    return Err(AgentError::ValidationFailed(format!(
                        "Step '{}' depends on non-existent step '{}'",
                        step.id, dep
                    )));
                }
            }
        }

        // Cycle detection via Kahn's algorithm (topological sort)
        self.detect_cycles()
    }

    /// Produce a topological ordering of step IDs.
    ///
    /// Returns `Err` if the dependency graph contains a cycle.
    pub fn topological_order(&self) -> AgentResult<Vec<String>> {
        let mut in_degree: HashMap<&str, usize> = HashMap::new();
        let mut adjacency: HashMap<&str, Vec<&str>> = HashMap::new();

        for step in &self.steps {
            in_degree.entry(step.id.as_str()).or_insert(0);
            adjacency.entry(step.id.as_str()).or_default();
            for dep in &step.depends_on {
                adjacency
                    .entry(dep.as_str())
                    .or_default()
                    .push(step.id.as_str());
                *in_degree.entry(step.id.as_str()).or_insert(0) += 1;
            }
        }

        let mut queue: Vec<&str> = in_degree
            .iter()
            .filter(|&(_, &deg)| deg == 0)
            .map(|(&id, _)| id)
            .collect();

        // Deterministic ordering for testability.
        queue.sort();

        let mut order = Vec::with_capacity(self.steps.len());

        while let Some(node) = queue.pop() {
            order.push(node.to_string());
            if let Some(children) = adjacency.get(node) {
                for &child in children {
                    let deg = in_degree.get_mut(child).unwrap();
                    *deg -= 1;
                    if *deg == 0 {
                        // Insert sorted to keep deterministic.
                        let pos = queue.binary_search(&child).unwrap_or_else(|p| p);
                        queue.insert(pos, child);
                    }
                }
            }
        }

        if order.len() != self.steps.len() {
            return Err(AgentError::ValidationFailed(
                "Plan dependency graph contains a cycle".into(),
            ));
        }

        Ok(order)
    }

    /// Internal helper that wraps `topological_order` for validation.
    fn detect_cycles(&self) -> AgentResult<()> {
        self.topological_order().map(|_| ())
    }
}

// ---------------------------------------------------------------------------
// Step Result (for synthesis)
// ---------------------------------------------------------------------------

/// Output of a successfully completed plan step, passed to the synthesis phase.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StepResult {
    /// Which step produced this result.
    pub step_id: String,
    /// The textual output of the step.
    pub output: String,
}

// ---------------------------------------------------------------------------
// Reflection
// ---------------------------------------------------------------------------

/// Verdict returned by [`Planner::reflect`] after evaluating a step's output.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub enum ReflectionVerdict {
    /// The step output meets the completion criterion — proceed.
    Accept,
    /// The step should be retried with the given feedback.
    Retry(String),
    /// The entire plan should be revised; includes the reason.
    Replan(String),
}

// ---------------------------------------------------------------------------
// Configuration
// ---------------------------------------------------------------------------

/// Configuration for the planning executor.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanningConfig {
    /// Maximum number of replanning iterations before failing.
    pub max_replans: u32,

    /// Maximum number of steps that may run concurrently.
    pub max_parallel_steps: usize,

    /// Per-step timeout in milliseconds (0 = no timeout).
    pub step_timeout_ms: u64,

    /// Default maximum retries per step (can be overridden per step).
    pub default_step_retries: u32,
}

impl Default for PlanningConfig {
    fn default() -> Self {
        Self {
            max_replans: 3,
            max_parallel_steps: 4,
            step_timeout_ms: 0,
            default_step_retries: 2,
        }
    }
}

impl PlanningConfig {
    /// Create a new config with defaults.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set maximum replan iterations.
    pub fn with_max_replans(mut self, max: u32) -> Self {
        self.max_replans = max;
        self
    }

    /// Set maximum parallel step concurrency.
    pub fn with_max_parallel_steps(mut self, max: usize) -> Self {
        self.max_parallel_steps = max;
        self
    }

    /// Set per-step timeout.
    pub fn with_step_timeout(mut self, timeout_ms: u64) -> Self {
        self.step_timeout_ms = timeout_ms;
        self
    }
}

// ---------------------------------------------------------------------------
// Planner Trait
// ---------------------------------------------------------------------------

/// Trait for goal decomposition, reflection, replanning, and synthesis.
///
/// Implementors decide *how* to break a goal into steps (e.g. via an LLM call,
/// a rule engine, or a static config). The planning executor calls these
/// methods at the appropriate phase of the loop.
///
/// # Provided implementations
///
/// - `LLMPlanner` (in `mofa-foundation`) — delegates to an LLM provider for
///   structured JSON plan generation, reflection, and synthesis.
#[async_trait::async_trait]
pub trait Planner: Send + Sync {
    /// Decompose a high-level goal into an executable [`Plan`].
    ///
    /// The returned plan must pass [`Plan::validate`] (no cycles, no
    /// dangling refs). The executor will call `validate()` before
    /// proceeding with execution.
    async fn decompose(&self, goal: &str) -> AgentResult<Plan>;

    /// Evaluate a step's output against its completion criterion.
    ///
    /// Called after each step finishes. Return:
    /// - [`ReflectionVerdict::Accept`] to proceed
    /// - [`ReflectionVerdict::Retry`] to re-run the step with feedback
    /// - [`ReflectionVerdict::Replan`] to generate a new plan
    async fn reflect(&self, step: &PlanStep, result: &str) -> AgentResult<ReflectionVerdict>;

    /// Generate a revised plan after a step failure.
    ///
    /// Receives the current (partially-completed) plan, the failed step,
    /// and the error message. Should produce a new plan that accounts for
    /// completed work and avoids the previous failure.
    async fn replan(&self, plan: &Plan, failed_step: &PlanStep, error: &str) -> AgentResult<Plan>;

    /// Combine all step results into a coherent final answer.
    async fn synthesize(&self, goal: &str, results: &[StepResult]) -> AgentResult<String>;
}

// ---------------------------------------------------------------------------
// Planning Stream Events
// ---------------------------------------------------------------------------

/// Events emitted during planning loop execution for observability.
///
/// These supplement the graph-level [`StreamEvent`](super::graph::StreamEvent)
/// to provide fine-grained visibility into the planning lifecycle.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub enum PlanningEvent {
    /// A new plan was created (or revised).
    PlanCreated { plan: Plan },

    /// A step started executing.
    StepStarted {
        step_id: String,
        description: String,
    },

    /// A step completed successfully.
    StepCompleted { step_id: String, result: String },

    /// A step failed.
    StepFailed {
        step_id: String,
        error: String,
        will_retry: bool,
    },

    /// A step is being retried.
    StepRetry {
        step_id: String,
        attempt: u32,
        feedback: String,
    },

    /// The plan is being revised.
    ReplanTriggered { iteration: u32, reason: String },

    /// Synthesis has started.
    SynthesisStarted { num_results: usize },

    /// The planning loop completed.
    PlanningComplete { final_answer: String },
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn step_status_default_is_pending() {
        let status = StepStatus::default();
        assert_eq!(status, StepStatus::Pending);
        assert!(!status.is_terminal());
        assert!(!status.is_success());
    }

    #[test]
    fn step_status_terminal_states() {
        assert!(StepStatus::Completed.is_terminal());
        assert!(StepStatus::Failed("err".into()).is_terminal());
        assert!(StepStatus::Skipped.is_terminal());
        assert!(!StepStatus::Pending.is_terminal());
        assert!(!StepStatus::Running.is_terminal());
    }

    #[test]
    fn plan_step_builder() {
        let step = PlanStep::new("search", "Search the web")
            .with_tool("web_search")
            .with_tool("browser")
            .depends_on("init")
            .with_criterion("Returns at least 3 results")
            .with_max_retries(5);

        assert_eq!(step.id, "search");
        assert_eq!(step.tools_needed, vec!["web_search", "browser"]);
        assert_eq!(step.depends_on, vec!["init"]);
        assert_eq!(step.completion_criterion, "Returns at least 3 results");
        assert_eq!(step.max_retries, 5);
        assert_eq!(step.status, StepStatus::Pending);
        assert!(step.can_retry());
    }

    #[test]
    fn plan_step_dependencies_met() {
        let step = PlanStep::new("b", "Step B").depends_on("a");
        let empty: HashSet<String> = HashSet::new();
        let with_a: HashSet<String> = ["a".to_string()].into();

        assert!(!step.dependencies_met(&empty));
        assert!(step.dependencies_met(&with_a));
    }

    #[test]
    fn plan_builder_and_lookup() {
        let plan = Plan::new("test goal")
            .add_step(PlanStep::new("a", "Step A"))
            .add_step(PlanStep::new("b", "Step B").depends_on("a"));

        assert_eq!(plan.steps.len(), 2);
        assert!(plan.get_step("a").is_some());
        assert!(plan.get_step("c").is_none());
    }

    #[test]
    fn plan_ready_steps() {
        let plan = Plan::new("goal")
            .add_step(PlanStep::new("a", "A"))
            .add_step(PlanStep::new("b", "B").depends_on("a"))
            .add_step(PlanStep::new("c", "C"));

        let empty: HashSet<String> = HashSet::new();
        let ready = plan.ready_steps(&empty);
        assert_eq!(ready.len(), 2); // a and c are ready
        assert!(ready.contains(&"a".to_string()));
        assert!(ready.contains(&"c".to_string()));

        let done_a: HashSet<String> = ["a".to_string()].into();
        let ready2 = plan.ready_steps(&done_a);
        assert!(ready2.contains(&"b".to_string()));
        assert!(ready2.contains(&"c".to_string()));
    }

    #[test]
    fn plan_validate_passes_for_valid_dag() {
        let plan = Plan::new("goal")
            .add_step(PlanStep::new("a", "A"))
            .add_step(PlanStep::new("b", "B").depends_on("a"))
            .add_step(PlanStep::new("c", "C").depends_on("a"))
            .add_step(PlanStep::new("d", "D").depends_on("b").depends_on("c"));

        assert!(plan.validate().is_ok());
    }

    #[test]
    fn plan_validate_detects_cycle() {
        let plan = Plan::new("goal")
            .add_step(PlanStep::new("a", "A").depends_on("c"))
            .add_step(PlanStep::new("b", "B").depends_on("a"))
            .add_step(PlanStep::new("c", "C").depends_on("b"));

        let err = plan.validate().unwrap_err();
        assert!(
            err.to_string().contains("cycle"),
            "Expected cycle error, got: {}",
            err
        );
    }

    #[test]
    fn plan_validate_detects_dangling_ref() {
        let plan = Plan::new("goal").add_step(PlanStep::new("a", "A").depends_on("missing"));

        let err = plan.validate().unwrap_err();
        assert!(
            err.to_string().contains("non-existent"),
            "Expected dangling ref error, got: {}",
            err
        );
    }

    #[test]
    fn plan_validate_detects_duplicate_ids() {
        let plan = Plan::new("goal")
            .add_step(PlanStep::new("a", "A"))
            .add_step(PlanStep::new("a", "A duplicate"));

        let err = plan.validate().unwrap_err();
        assert!(
            err.to_string().contains("duplicate"),
            "Expected duplicate error, got: {}",
            err
        );
    }

    #[test]
    fn plan_topological_order_linear() {
        let plan = Plan::new("goal")
            .add_step(PlanStep::new("a", "A"))
            .add_step(PlanStep::new("b", "B").depends_on("a"))
            .add_step(PlanStep::new("c", "C").depends_on("b"));

        let order = plan.topological_order().unwrap();
        assert_eq!(order, vec!["a", "b", "c"]);
    }

    #[test]
    fn plan_topological_order_diamond() {
        // a -> b,c -> d
        let plan = Plan::new("goal")
            .add_step(PlanStep::new("a", "A"))
            .add_step(PlanStep::new("b", "B").depends_on("a"))
            .add_step(PlanStep::new("c", "C").depends_on("a"))
            .add_step(PlanStep::new("d", "D").depends_on("b").depends_on("c"));

        let order = plan.topological_order().unwrap();
        // a must come first, d must come last, b and c can be either order
        assert_eq!(order[0], "a");
        assert_eq!(order[3], "d");
        assert!(order[1] == "b" || order[1] == "c");
        assert!(order[2] == "b" || order[2] == "c");
    }

    #[test]
    fn plan_serialization_roundtrip() {
        let plan = Plan::new("test goal")
            .add_step(
                PlanStep::new("search", "Search the web")
                    .with_tool("web_search")
                    .with_criterion("Returns results"),
            )
            .add_step(
                PlanStep::new("analyze", "Analyze results")
                    .depends_on("search")
                    .with_max_retries(3),
            );

        let json = serde_json::to_string_pretty(&plan).expect("serialize");
        let deserialized: Plan = serde_json::from_str(&json).expect("deserialize");

        assert_eq!(deserialized.goal, plan.goal);
        assert_eq!(deserialized.steps.len(), 2);
        assert_eq!(deserialized.steps[0].id, "search");
        assert_eq!(deserialized.steps[1].depends_on, vec!["search"]);
    }

    #[test]
    fn reflection_verdict_serialization() {
        let accept = ReflectionVerdict::Accept;
        let retry = ReflectionVerdict::Retry("Try again with more detail".into());
        let replan = ReflectionVerdict::Replan("Step approach was wrong".into());

        // Ensure they all serialize without panicking
        let _ = serde_json::to_string(&accept).unwrap();
        let _ = serde_json::to_string(&retry).unwrap();
        let _ = serde_json::to_string(&replan).unwrap();
    }

    #[test]
    fn planning_config_defaults() {
        let config = PlanningConfig::default();
        assert_eq!(config.max_replans, 3);
        assert_eq!(config.max_parallel_steps, 4);
        assert_eq!(config.step_timeout_ms, 0);
        assert_eq!(config.default_step_retries, 2);
    }

    #[test]
    fn planning_config_builder() {
        let config = PlanningConfig::new()
            .with_max_replans(5)
            .with_max_parallel_steps(8)
            .with_step_timeout(30000);

        assert_eq!(config.max_replans, 5);
        assert_eq!(config.max_parallel_steps, 8);
        assert_eq!(config.step_timeout_ms, 30000);
    }

    #[test]
    fn plan_completed_results() {
        let mut plan = Plan::new("goal")
            .add_step(PlanStep::new("a", "A"))
            .add_step(PlanStep::new("b", "B"))
            .add_step(PlanStep::new("c", "C"));

        plan.steps[0].status = StepStatus::Completed;
        plan.steps[0].result = Some("Result A".into());
        plan.steps[1].status = StepStatus::Failed("oops".into());
        plan.steps[2].status = StepStatus::Completed;
        plan.steps[2].result = Some("Result C".into());

        let results = plan.completed_results();
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].step_id, "a");
        assert_eq!(results[1].step_id, "c");
    }

    #[test]
    fn plan_is_complete() {
        let mut plan = Plan::new("goal")
            .add_step(PlanStep::new("a", "A"))
            .add_step(PlanStep::new("b", "B"));

        assert!(!plan.is_complete());

        plan.steps[0].status = StepStatus::Completed;
        assert!(!plan.is_complete());

        plan.steps[1].status = StepStatus::Skipped;
        assert!(plan.is_complete());
    }

    #[test]
    fn planning_event_serialization() {
        let event = PlanningEvent::StepCompleted {
            step_id: "search".into(),
            result: "Found 5 results".into(),
        };

        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("StepCompleted"));
        assert!(json.contains("search"));
    }
}
