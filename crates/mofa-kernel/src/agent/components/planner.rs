//! Planning component.
//!
//! Kernel-level contracts for decomposing a goal into executable steps.
//! Concrete implementations live in the foundation layer.

use crate::agent::components::tool::ToolDescriptor;
use crate::agent::context::AgentContext;
use crate::agent::error::AgentResult;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

/// Trait for planning: decompose a goal into an ordered set of executable steps.
#[async_trait]
pub trait Planner: Send + Sync {
    /// Produce an execution plan for the given request.
    async fn plan(
        &self,
        request: &PlanningRequest,
        ctx: &AgentContext,
    ) -> AgentResult<ExecutionPlan>;

    /// Name of the planner implementation.
    fn name(&self) -> &str {
        "planner"
    }

    /// Whether this planner can revise an existing plan.
    fn supports_replanning(&self) -> bool {
        true
    }
}

/// Input to the planner.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PlanningRequest {
    /// High-level goal to fulfil.
    pub goal: String,
    /// Tools available for step assignment.
    pub available_tools: Vec<ToolDescriptor>,
    /// Relevant memory items recalled for this goal.
    pub recalled_memory: Vec<MemoryItemSnapshot>,
    /// Steps already completed (used during re-planning).
    pub completed_steps: Vec<CompletedStep>,
    /// Existing plan being revised (populated on re-plan).
    pub prior_plan: Option<ExecutionPlan>,
    /// Reason the prior plan needs revision.
    pub replan_reason: Option<String>,
}

impl PlanningRequest {
    /// Build a request for a fresh goal with no prior context.
    pub fn new(goal: impl Into<String>) -> Self {
        Self {
            goal: goal.into(),
            ..Self::default()
        }
    }
}

/// A recalled memory item passed to the planner for context.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MemoryItemSnapshot {
    /// Storage key.
    pub key: String,
    /// Plain-text content the planner can read.
    pub content: String,
    /// Optional metadata labels.
    pub metadata: Vec<(String, String)>,
}

impl MemoryItemSnapshot {
    /// Construct a snapshot.
    pub fn new(
        key: impl Into<String>,
        content: impl Into<String>,
        metadata: Vec<(String, String)>,
    ) -> Self {
        Self {
            key: key.into(),
            content: content.into(),
            metadata,
        }
    }
}

/// Summary of a step that completed before a re-plan.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CompletedStep {
    /// Step identifier.
    pub id: String,
    /// Output produced by the step.
    pub output: String,
}

impl CompletedStep {
    /// Construct a completed-step record.
    pub fn new(id: impl Into<String>, output: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            output: output.into(),
        }
    }
}

/// A full execution plan returned by a planner.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct ExecutionPlan {
    /// Goal the plan addresses.
    pub goal: String,
    /// Ordered list of steps (dependencies determine actual execution order).
    pub steps: Vec<PlannedStep>,
}

impl ExecutionPlan {
    /// Validate dependency references and detect cycles.
    pub fn validate(&self) -> AgentResult<()> {
        use crate::agent::error::AgentError;

        let known: HashSet<&str> = self.steps.iter().map(|s| s.id.as_str()).collect();

        for step in &self.steps {
            if step.id.trim().is_empty() {
                return Err(AgentError::ValidationFailed(
                    "planning step id cannot be empty".to_string(),
                ));
            }
            for dep in &step.dependencies {
                if dep == &step.id {
                    return Err(AgentError::ValidationFailed(format!(
                        "planning step '{}' cannot depend on itself",
                        step.id
                    )));
                }
                if !known.contains(dep.as_str()) {
                    return Err(AgentError::ValidationFailed(format!(
                        "planning step '{}' depends on unknown step '{dep}'",
                        step.id
                    )));
                }
            }
        }

        self.topological_groups().map(|_| ())
    }

    /// Group steps by dependency depth; each group may run in parallel.
    ///
    /// Returns `Err` if the dependency graph contains a cycle.
    pub fn topological_groups(&self) -> AgentResult<Vec<Vec<PlannedStep>>> {
        use crate::agent::error::AgentError;

        let mut pending = self.steps.clone();
        let mut completed: HashSet<String> = HashSet::new();
        let mut groups: Vec<Vec<PlannedStep>> = Vec::new();

        while !pending.is_empty() {
            let (ready, blocked): (Vec<_>, Vec<_>) = pending.into_iter().partition(|step| {
                step.dependencies
                    .iter()
                    .all(|dep| completed.contains(dep))
            });

            if ready.is_empty() {
                return Err(AgentError::ValidationFailed(
                    "planning dependency graph contains a cycle".to_string(),
                ));
            }

            for step in &ready {
                completed.insert(step.id.clone());
            }

            pending = blocked;
            groups.push(ready);
        }

        Ok(groups)
    }
}

/// A single step inside an execution plan.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct PlannedStep {
    /// Stable, unique identifier within the plan.
    pub id: String,
    /// Human-readable description of what this step does.
    pub description: String,
    /// IDs of steps that must complete before this one starts.
    pub dependencies: Vec<String>,
    /// Names of tools this step expects to invoke.
    pub required_tools: Vec<String>,
    /// Inputs (from prior steps or the goal) the executor should supply.
    pub expected_inputs: Vec<String>,
    /// Criterion that the reflection phase validates.
    pub completion_criterion: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_step(id: &str, deps: &[&str]) -> PlannedStep {
        PlannedStep {
            id: id.to_string(),
            dependencies: deps.iter().map(|d| d.to_string()).collect(),
            ..Default::default()
        }
    }

    #[test]
    fn topological_groups_linear_chain() {
        let plan = ExecutionPlan {
            goal: "test".to_string(),
            steps: vec![make_step("a", &[]), make_step("b", &["a"]), make_step("c", &["b"])],
        };

        let groups = plan.topological_groups().unwrap();
        assert_eq!(groups.len(), 3);
        assert_eq!(groups[0][0].id, "a");
        assert_eq!(groups[1][0].id, "b");
        assert_eq!(groups[2][0].id, "c");
    }

    #[test]
    fn topological_groups_parallel_steps() {
        let plan = ExecutionPlan {
            goal: "test".to_string(),
            steps: vec![
                make_step("a", &[]),
                make_step("b", &[]),
                make_step("c", &["a", "b"]),
            ],
        };

        let groups = plan.topological_groups().unwrap();
        assert_eq!(groups.len(), 2);
        assert_eq!(groups[0].len(), 2);
        assert_eq!(groups[1][0].id, "c");
    }

    #[test]
    fn topological_groups_detects_cycle() {
        let plan = ExecutionPlan {
            goal: "test".to_string(),
            steps: vec![make_step("a", &["b"]), make_step("b", &["a"])],
        };
        assert!(plan.topological_groups().is_err());
    }

    #[test]
    fn validate_rejects_self_dependency() {
        let plan = ExecutionPlan {
            goal: "test".to_string(),
            steps: vec![make_step("a", &["a"])],
        };
        assert!(plan.validate().is_err());
    }

    #[test]
    fn validate_rejects_unknown_dependency() {
        let plan = ExecutionPlan {
            goal: "test".to_string(),
            steps: vec![make_step("a", &["nonexistent"])],
        };
        assert!(plan.validate().is_err());
    }

    #[test]
    fn validate_rejects_empty_step_id() {
        let plan = ExecutionPlan {
            goal: "test".to_string(),
            steps: vec![make_step("", &[])],
        };
        assert!(plan.validate().is_err());
    }
}
