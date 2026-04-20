//! SwarmComposer — cost-aware agent assignment for swarm tasks.
//!
//! Matches each subtask's `required_capabilities` against the registered
//! `AgentSpec` pool and picks the cheapest agent that:
//!
//! 1. Covers all (or the most) required capabilities
//! 2. Keeps the cumulative estimated cost within `SLAConfig::max_cost_tokens`
//!
//! ## Assignment algorithm
//!
//! ```text
//!  for each task in DAG (topological order):
//!    1. find agents with full capability coverage  ──┐
//!    2. if none, find agents with best partial cover  │ candidates
//!    3. filter agents that would exceed SLA budget  ──┘
//!    4. sort by cost_per_token ascending (cheapest first)
//!    5. assign cheapest; update cumulative cost tracker
//! ```
//!
//! ## Cost estimation
//!
//! Exact token counts are unknown at planning time. The composer uses:
//!
//! ```text
//! estimated_tokens = task.complexity × TOKENS_PER_COMPLEXITY_UNIT
//! estimated_cost   = estimated_tokens × agent.cost_per_token
//! ```
//!
//! When `SLAConfig::max_cost_tokens == 0` the budget is treated as unlimited.

use std::collections::HashMap;

use crate::swarm::config::{AgentSpec, SLAConfig, SwarmConfig};
use crate::swarm::dag::{SubtaskDAG, SwarmSubtask};

/// estimated tokens a task with complexity 1.0 will consume
const TOKENS_PER_COMPLEXITY_UNIT: f64 = 1_000.0;

/// assignment result for a single subtask
#[derive(Debug, Clone)]
pub struct AgentAssignment {
    /// id of the agent assigned to this task
    pub agent_id: String,
    /// fraction of required capabilities covered (1.0 = full match)
    pub capability_coverage: f64,
    /// estimated token cost for this assignment
    pub estimated_cost_tokens: f64,
}

/// warning emitted when an assignment exceeds or approaches the SLA budget
#[derive(Debug, Clone)]
pub struct BudgetWarning {
    pub task_id: String,
    pub agent_id: String,
    pub estimated_cost: f64,
    pub remaining_budget: f64,
}

/// result returned by `SwarmComposer::assign`
#[derive(Debug, Default)]
pub struct ComposerResult {
    /// task_id -> assignment for all successfully assigned tasks
    pub assignments: HashMap<String, AgentAssignment>,
    /// task ids for which no matching agent was found
    pub unassigned: Vec<String>,
    /// cumulative estimated token cost across all assignments
    pub estimated_total_cost_tokens: f64,
    /// warnings when assignments push against the SLA budget
    pub budget_warnings: Vec<BudgetWarning>,
}

impl ComposerResult {
    /// returns true when every task was assigned
    pub fn is_fully_assigned(&self) -> bool {
        self.unassigned.is_empty()
    }

    /// fraction of tasks that were assigned (0.0–1.0)
    pub fn coverage_ratio(&self) -> f64 {
        let total = self.assignments.len() + self.unassigned.len();
        if total == 0 {
            return 1.0;
        }
        self.assignments.len() as f64 / total as f64
    }
}

/// Cost-aware agent assignment engine.
///
/// Instantiate from a `SwarmConfig` or from raw `AgentSpec` + `SLAConfig`, then
/// call `assign` to annotate every subtask in a DAG with an optimal agent.
///
/// # Example
///
/// ```rust,ignore
/// let composer = SwarmComposer::from_config(&swarm_config);
/// let result = composer.assign(&mut dag);
///
/// if !result.is_fully_assigned() {
///     eprintln!("unassigned: {:?}", result.unassigned);
/// }
/// ```
#[derive(Debug, Clone)]
pub struct SwarmComposer {
    agents: Vec<AgentSpec>,
    sla: SLAConfig,
}

impl SwarmComposer {
    pub fn new(agents: Vec<AgentSpec>, sla: SLAConfig) -> Self {
        Self { agents, sla }
    }

    pub fn from_config(config: &SwarmConfig) -> Self {
        Self::new(config.agents.clone(), config.sla.clone())
    }

    /// Assign the cheapest valid agent to every task in `dag`.
    ///
    /// Writes `task.assigned_agent` for each assigned task.
    /// Returns a `ComposerResult` with full assignment details and budget status.
    pub fn assign(&self, dag: &mut SubtaskDAG) -> ComposerResult {
        let mut result = ComposerResult::default();
        let budget = if self.sla.max_cost_tokens == 0 {
            f64::INFINITY
        } else {
            self.sla.max_cost_tokens as f64
        };

        // process in topological order so earlier tasks are assigned first
        let order = dag.topological_order().unwrap_or_else(|_| {
            dag.all_tasks().into_iter().map(|(idx, _)| idx).collect()
        });

        for idx in order {
            if let Some(task) = dag.get_task(idx) {
                let task = task.clone();
                let remaining = budget - result.estimated_total_cost_tokens;

                match self.best_assignment(&task, remaining) {
                    Some(assignment) => {
                        // warn if this assignment consumes > 80% of remaining budget
                        if budget.is_finite()
                            && assignment.estimated_cost_tokens > remaining * 0.8
                        {
                            result.budget_warnings.push(BudgetWarning {
                                task_id: task.id.clone(),
                                agent_id: assignment.agent_id.clone(),
                                estimated_cost: assignment.estimated_cost_tokens,
                                remaining_budget: remaining,
                            });
                        }

                        result.estimated_total_cost_tokens += assignment.estimated_cost_tokens;
                        result.assignments.insert(task.id.clone(), assignment.clone());

                        // write back to the DAG
                        if let Some(t) = dag.get_task_mut(idx) {
                            t.assigned_agent = Some(assignment.agent_id);
                        }
                    }
                    None => {
                        result.unassigned.push(task.id.clone());
                    }
                }
            }
        }

        result
    }

    /// Find the best agent for a single task given the remaining budget.
    ///
    /// Prefers agents with full capability coverage; falls back to best partial.
    /// Among equally-covering agents, selects the cheapest.
    pub fn best_assignment(
        &self,
        task: &SwarmSubtask,
        remaining_budget: f64,
    ) -> Option<AgentAssignment> {
        if self.agents.is_empty() {
            return None;
        }

        let required = &task.required_capabilities;

        // score each agent: (coverage fraction, negative cost) for sorting
        let mut candidates: Vec<(&AgentSpec, f64, f64)> = self
            .agents
            .iter()
            .filter_map(|agent| {
                let coverage = capability_coverage(required, &agent.capabilities);
                if coverage == 0.0 && !required.is_empty() {
                    return None; // zero overlap — not a candidate
                }
                let cost = estimated_cost(task.complexity, agent);
                // respect SLA budget — skip agents that would blow it
                if cost > remaining_budget {
                    return None;
                }
                Some((agent, coverage, cost))
            })
            .collect();

        if candidates.is_empty() {
            // budget exceeded for all candidates — relax budget constraint and
            // return the cheapest partial match with a warning implicit in ComposerResult
            candidates = self
                .agents
                .iter()
                .filter_map(|agent| {
                    let coverage = capability_coverage(required, &agent.capabilities);
                    if coverage == 0.0 && !required.is_empty() {
                        return None;
                    }
                    let cost = estimated_cost(task.complexity, agent);
                    Some((agent, coverage, cost))
                })
                .collect();

            if candidates.is_empty() {
                return None;
            }
        }

        // sort: higher coverage first, then lower cost
        candidates.sort_by(|a, b| {
            b.1.partial_cmp(&a.1)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then(a.2.partial_cmp(&b.2).unwrap_or(std::cmp::Ordering::Equal))
        });

        let (best_agent, coverage, cost) = candidates.into_iter().next()?;

        Some(AgentAssignment {
            agent_id: best_agent.id.clone(),
            capability_coverage: coverage,
            estimated_cost_tokens: cost,
        })
    }
}

/// fraction of `required` capabilities covered by `available` (0.0–1.0)
fn capability_coverage(required: &[String], available: &[String]) -> f64 {
    if required.is_empty() {
        return 1.0; // no requirements — any agent qualifies
    }
    let matched = required
        .iter()
        .filter(|r| available.iter().any(|a| a.eq_ignore_ascii_case(r)))
        .count();
    matched as f64 / required.len() as f64
}

/// estimated token cost for assigning `agent` to a task with this complexity
fn estimated_cost(complexity: f64, agent: &AgentSpec) -> f64 {
    let tokens = complexity.clamp(0.0, 1.0) * TOKENS_PER_COMPLEXITY_UNIT;
    agent.cost_per_token.unwrap_or(0.0) * tokens
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::swarm::dag::{SubtaskDAG, SwarmSubtask};

    fn agent(id: &str, caps: &[&str], cost: f64) -> AgentSpec {
        AgentSpec {
            id: id.to_string(),
            capabilities: caps.iter().map(|s| s.to_string()).collect(),
            model: None,
            cost_per_token: Some(cost),
            max_concurrency: 1,
        }
    }

    fn task(id: &str, caps: &[&str], complexity: f64) -> SwarmSubtask {
        SwarmSubtask::new(id, id)
            .with_capabilities(caps.iter().map(|s| s.to_string()).collect())
            .with_complexity(complexity)
    }

    // --- capability_coverage ---

    #[test]
    fn full_coverage_returns_one() {
        let req = vec!["a".to_string(), "b".to_string()];
        let avail = vec!["a".to_string(), "b".to_string(), "c".to_string()];
        assert_eq!(capability_coverage(&req, &avail), 1.0);
    }

    #[test]
    fn partial_coverage() {
        let req = vec!["a".to_string(), "b".to_string(), "c".to_string()];
        let avail = vec!["a".to_string(), "c".to_string()];
        let cov = capability_coverage(&req, &avail);
        assert!((cov - 2.0 / 3.0).abs() < 1e-9);
    }

    #[test]
    fn no_requirements_returns_full_coverage() {
        assert_eq!(capability_coverage(&[], &["x".to_string()]), 1.0);
    }

    #[test]
    fn zero_coverage_when_no_overlap() {
        let req = vec!["x".to_string()];
        let avail = vec!["y".to_string()];
        assert_eq!(capability_coverage(&req, &avail), 0.0);
    }

    #[test]
    fn coverage_is_case_insensitive() {
        let req = vec!["WebSearch".to_string()];
        let avail = vec!["websearch".to_string()];
        assert_eq!(capability_coverage(&req, &avail), 1.0);
    }

    // --- best_assignment ---

    #[test]
    fn picks_cheapest_fully_covering_agent() {
        let composer = SwarmComposer::new(
            vec![
                agent("expensive", &["search", "summarize"], 0.01),
                agent("cheap", &["search", "summarize"], 0.001),
            ],
            SLAConfig::default(),
        );
        let t = task("t1", &["search", "summarize"], 0.5);
        let assignment = composer.best_assignment(&t, f64::INFINITY).unwrap();
        assert_eq!(assignment.agent_id, "cheap");
        assert_eq!(assignment.capability_coverage, 1.0);
    }

    #[test]
    fn prefers_full_coverage_over_cheap_partial() {
        let composer = SwarmComposer::new(
            vec![
                agent("partial-cheap", &["search"], 0.0001),
                agent("full-expensive", &["search", "summarize"], 0.01),
            ],
            SLAConfig::default(),
        );
        let t = task("t1", &["search", "summarize"], 0.5);
        let assignment = composer.best_assignment(&t, f64::INFINITY).unwrap();
        assert_eq!(assignment.agent_id, "full-expensive");
    }

    #[test]
    fn respects_budget_limit() {
        // cheap agent costs 0.001 * 500 = 0.5 tokens
        // expensive agent costs 0.01 * 500 = 5.0 tokens
        let composer = SwarmComposer::new(
            vec![
                agent("cheap", &["search"], 0.001),
                agent("expensive", &["search"], 0.01),
            ],
            SLAConfig::default(),
        );
        let t = task("t1", &["search"], 0.5);
        // budget of 1.0 — expensive (5.0) exceeds it, cheap (0.5) does not
        let assignment = composer.best_assignment(&t, 1.0).unwrap();
        assert_eq!(assignment.agent_id, "cheap");
    }

    #[test]
    fn returns_none_when_no_agents() {
        let composer = SwarmComposer::new(vec![], SLAConfig::default());
        let t = task("t1", &["search"], 0.5);
        assert!(composer.best_assignment(&t, f64::INFINITY).is_none());
    }

    #[test]
    fn returns_none_when_no_capability_overlap() {
        let composer = SwarmComposer::new(
            vec![agent("a1", &["coding"], 0.001)],
            SLAConfig::default(),
        );
        let t = task("t1", &["search"], 0.5);
        assert!(composer.best_assignment(&t, f64::INFINITY).is_none());
    }

    #[test]
    fn task_with_no_requirements_assigns_cheapest_agent() {
        let composer = SwarmComposer::new(
            vec![
                agent("a1", &["x"], 0.01),
                agent("a2", &["y"], 0.001),
            ],
            SLAConfig::default(),
        );
        let t = task("t1", &[], 0.5); // no requirements
        let assignment = composer.best_assignment(&t, f64::INFINITY).unwrap();
        assert_eq!(assignment.agent_id, "a2"); // cheapest
        assert_eq!(assignment.capability_coverage, 1.0);
    }

    // --- assign (DAG-level) ---

    #[test]
    fn assign_writes_agent_to_dag() {
        let mut dag = SubtaskDAG::new("test");
        dag.add_task(task("t1", &["search"], 0.5));
        dag.add_task(task("t2", &["coding"], 0.5));

        let composer = SwarmComposer::new(
            vec![
                agent("searcher", &["search"], 0.001),
                agent("coder", &["coding"], 0.002),
            ],
            SLAConfig::default(),
        );

        let result = composer.assign(&mut dag);

        assert!(result.is_fully_assigned());
        assert_eq!(result.assignments.len(), 2);

        // verify assignments written back to DAG
        for (_, task) in dag.all_tasks() {
            assert!(task.assigned_agent.is_some(), "every task should be assigned");
        }
    }

    #[test]
    fn assign_tracks_unassigned_when_no_match() {
        let mut dag = SubtaskDAG::new("test");
        dag.add_task(task("t1", &["quantum-physics"], 0.5));

        let composer = SwarmComposer::new(
            vec![agent("coder", &["coding"], 0.001)],
            SLAConfig::default(),
        );

        let result = composer.assign(&mut dag);
        assert!(!result.is_fully_assigned());
        assert_eq!(result.unassigned, vec!["t1"]);
        assert_eq!(result.coverage_ratio(), 0.0);
    }

    #[test]
    fn assign_respects_sla_budget_across_tasks() {
        let mut dag = SubtaskDAG::new("budget-test");
        // complexity 0.5 → 500 tokens per task
        // cheap agent: 0.001 * 500 = 0.5 per task
        // expensive: 0.01 * 500 = 5.0 per task
        dag.add_task(task("t1", &["search"], 0.5));
        dag.add_task(task("t2", &["search"], 0.5));

        let sla = SLAConfig {
            max_cost_tokens: 2, // budget = 2.0 tokens
            ..Default::default()
        };
        let composer = SwarmComposer::new(
            vec![
                agent("cheap", &["search"], 0.001),
                agent("expensive", &["search"], 0.01),
            ],
            sla,
        );

        let result = composer.assign(&mut dag);
        // both tasks assigned to cheap agent (0.5 each = 1.0 total < 2.0)
        assert!(result.is_fully_assigned());
        for (_, assignment) in &result.assignments {
            assert_eq!(assignment.agent_id, "cheap");
        }
    }

    #[test]
    fn coverage_ratio_all_assigned() {
        let result = ComposerResult {
            assignments: [("t1".to_string(), AgentAssignment {
                agent_id: "a".to_string(),
                capability_coverage: 1.0,
                estimated_cost_tokens: 1.0,
            })].into(),
            unassigned: vec![],
            estimated_total_cost_tokens: 1.0,
            budget_warnings: vec![],
        };
        assert_eq!(result.coverage_ratio(), 1.0);
    }

    #[test]
    fn coverage_ratio_half_assigned() {
        let result = ComposerResult {
            assignments: [("t1".to_string(), AgentAssignment {
                agent_id: "a".to_string(),
                capability_coverage: 1.0,
                estimated_cost_tokens: 1.0,
            })].into(),
            unassigned: vec!["t2".to_string()],
            estimated_total_cost_tokens: 1.0,
            budget_warnings: vec![],
        };
        assert_eq!(result.coverage_ratio(), 0.5);
    }

    #[test]
    fn budget_warning_emitted_when_cost_near_limit() {
        let mut dag = SubtaskDAG::new("warn-test");
        // complexity 1.0 → 1000 tokens; cost = 0.01 * 1000 = 10.0
        dag.add_task(task("t1", &["a"], 1.0));

        let sla = SLAConfig {
            max_cost_tokens: 11, // remaining = 11.0; cost 10.0 > 11.0 * 0.8 = 8.8
            ..Default::default()
        };
        let composer = SwarmComposer::new(
            vec![agent("a1", &["a"], 0.01)],
            sla,
        );

        let result = composer.assign(&mut dag);
        assert!(result.is_fully_assigned());
        assert_eq!(result.budget_warnings.len(), 1);
    }

    #[test]
    fn from_config_constructs_composer() {
        use crate::swarm::config::SwarmConfig;
        let config = SwarmConfig {
            id: "c1".to_string(),
            name: "test".to_string(),
            description: String::new(),
            task: "do things".to_string(),
            agents: vec![agent("a1", &["x"], 0.001)],
            pattern: Default::default(),
            sla: SLAConfig::default(),
            hitl: Default::default(),
            metadata: Default::default(),
        };
        let composer = SwarmComposer::from_config(&config);
        assert_eq!(composer.agents.len(), 1);
    }
}
