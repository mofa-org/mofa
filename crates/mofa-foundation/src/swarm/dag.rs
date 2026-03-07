//! SubtaskDAG: Directed Acyclic Graph for task decomposition

use chrono::{DateTime, Utc};
use petgraph::graph::{DiGraph, NodeIndex};
use petgraph::visit::EdgeRef;
use petgraph::Direction;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

use mofa_kernel::agent::types::error::{GlobalError, GlobalResult};

// SwarmSubtask Types
/// Status of an individual subtask in the DAG
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SubtaskStatus {
    Pending,
    Ready,
    Running,
    Completed,
    Failed(String),
    Skipped,
}

impl Default for SubtaskStatus {
    fn default() -> Self {
        Self::Pending
    }
}

/// A single subtask node in the DAG
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SwarmSubtask {
    pub id: String,
    pub description: String,
    pub required_capabilities: Vec<String>,
    pub status: SubtaskStatus,
    pub assigned_agent: Option<String>,
    pub output: Option<String>,
    pub complexity: f64,
    pub started_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
}

impl SwarmSubtask {
    /// Create a new subtask with the given id and description
    pub fn new(id: impl Into<String>, description: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            description: description.into(),
            required_capabilities: Vec::new(),
            status: SubtaskStatus::Pending,
            assigned_agent: None,
            output: None,
            complexity: 0.5,
            started_at: None,
            completed_at: None,
        }
    }

    /// Set required capabilities for this subtask
    pub fn with_capabilities(mut self, caps: Vec<String>) -> Self {
        self.required_capabilities = caps;
        self
    }

    /// Set the estimated complexity
    pub fn with_complexity(mut self, complexity: f64) -> Self {
        self.complexity = complexity.clamp(0.0, 1.0);
        self
    }
}

/// Edge metadata representing a dependency between subtasks
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DependencyEdge {
    /// What kind of dependency this is
    pub kind: DependencyKind,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DependencyKind {
    Sequential,
    DataFlow,
    Soft,
}

impl Default for DependencyEdge {
    fn default() -> Self {
        Self {
            kind: DependencyKind::Sequential,
        }
    }
}

// SubtaskDAG
/// Directed Acyclic Graph representing a decomposed task
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubtaskDAG {
    pub id: String,
    pub name: String,
    graph: DiGraph<SwarmSubtask, DependencyEdge>,
    #[serde(skip)]
    id_to_index: HashMap<String, NodeIndex>,
}

impl SubtaskDAG {
    /// Create a new empty DAG
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            id: Uuid::now_v7().to_string(),
            name: name.into(),
            graph: DiGraph::new(),
            id_to_index: HashMap::new(),
        }
    }

    /// Add a subtask to the DAG, returns its node index
    pub fn add_task(&mut self, task: SwarmSubtask) -> NodeIndex {
        let id = task.id.clone();
        let idx = self.graph.add_node(task);
        self.id_to_index.insert(id, idx);
        idx
    }

    /// Add a dependency edge: `from` must complete before `to` can start
    pub fn add_dependency(&mut self, from: NodeIndex, to: NodeIndex) -> GlobalResult<()> {
        self.add_dependency_with_kind(from, to, DependencyKind::Sequential)
    }

    /// Add a dependency edge with a specific kind
    pub fn add_dependency_with_kind(
        &mut self,
        from: NodeIndex,
        to: NodeIndex,
        kind: DependencyKind,
    ) -> GlobalResult<()> {
        self.graph.add_edge(from, to, DependencyEdge { kind });
        if petgraph::algo::is_cyclic_directed(&self.graph) {
            if let Some(edge) = self.graph.find_edge(from, to) {
                self.graph.remove_edge(edge);
            }
            return Err(GlobalError::Other(format!(
                "Adding dependency from {:?} to {:?} would create a cycle",
                from, to
            )));
        }

        Ok(())
    }

    /// Return tasks that are pending and have all hard dependencies satisfied
    pub fn ready_tasks(&self) -> Vec<NodeIndex> {
        self.graph
            .node_indices()
            .filter(|&idx| {
                let task = &self.graph[idx];
                if task.status != SubtaskStatus::Pending {
                    return false;
                }
                self.graph
                    .edges_directed(idx, Direction::Incoming)
                    .all(|edge| {
                        let dep = &self.graph[edge.source()];
                        let dep_edge = edge.weight();
                        match dep_edge.kind {
                            DependencyKind::Sequential | DependencyKind::DataFlow => {
                                dep.status == SubtaskStatus::Completed
                                    || dep.status == SubtaskStatus::Skipped
                            }
                            DependencyKind::Soft => true,
                        }
                    })
            })
            .collect()
    }

    /// Mark a task as running and record its start time
    pub fn mark_running(&mut self, idx: NodeIndex) {
        if let Some(task) = self.graph.node_weight_mut(idx) {
            task.status = SubtaskStatus::Running;
            task.started_at = Some(Utc::now());
        }
    }

    /// Mark a task as completed
    pub fn mark_complete(&mut self, idx: NodeIndex) {
        self.mark_complete_with_output(idx, None);
    }

    /// Mark a task as completed and attach its output
    pub fn mark_complete_with_output(&mut self, idx: NodeIndex, output: Option<String>) {
        if let Some(task) = self.graph.node_weight_mut(idx) {
            task.status = SubtaskStatus::Completed;
            task.completed_at = Some(Utc::now());
            task.output = output;
        }
    }

    /// Mark a task as failed with a reason string
    pub fn mark_failed(&mut self, idx: NodeIndex, reason: impl Into<String>) {
        if let Some(task) = self.graph.node_weight_mut(idx) {
            task.status = SubtaskStatus::Failed(reason.into());
            task.completed_at = Some(Utc::now());
        }
    }

    /// Mark a task as skipped
    pub fn mark_skipped(&mut self, idx: NodeIndex) {
        if let Some(task) = self.graph.node_weight_mut(idx) {
            task.status = SubtaskStatus::Skipped;
            task.completed_at = Some(Utc::now());
        }
    }

    /// Check if all tasks are completed (or skipped/failed)
    pub fn is_complete(&self) -> bool {
        self.graph.node_weights().all(|task| {
            matches!(
                task.status,
                SubtaskStatus::Completed | SubtaskStatus::Skipped | SubtaskStatus::Failed(_)
            )
        })
    }

    /// Get the topological execution order
    pub fn topological_order(&self) -> GlobalResult<Vec<NodeIndex>> {
        petgraph::algo::toposort(&self.graph, None).map_err(|cycle| {
            GlobalError::Other(format!("DAG contains a cycle at node {:?}", cycle.node_id()))
        })
    }

    /// Get a subtask by its node index
    pub fn get_task(&self, idx: NodeIndex) -> Option<&SwarmSubtask> {
        self.graph.node_weight(idx)
    }

    /// Get a mutable reference to a subtask
    pub fn get_task_mut(&mut self, idx: NodeIndex) -> Option<&mut SwarmSubtask> {
        self.graph.node_weight_mut(idx)
    }

    /// Look up a node index by subtask id
    pub fn find_by_id(&self, id: &str) -> Option<NodeIndex> {
        self.id_to_index.get(id).copied()
    }

    /// Total number of tasks in the DAG
    pub fn task_count(&self) -> usize {
        self.graph.node_count()
    }

    /// Number of tasks in the Completed state
    pub fn completed_count(&self) -> usize {
        self.graph
            .node_weights()
            .filter(|t| t.status == SubtaskStatus::Completed)
            .count()
    }

    /// Fraction of tasks completed
    pub fn progress(&self) -> f64 {
        let total = self.task_count();
        if total == 0 {
            return 1.0;
        }
        self.completed_count() as f64 / total as f64
    }

    /// Iterate over all tasks with their node indices
    pub fn all_tasks(&self) -> Vec<(NodeIndex, &SwarmSubtask)> {
        self.graph
            .node_indices()
            .map(|idx| (idx, &self.graph[idx]))
            .collect()
    }

    /// Get the dependencies of a specific task (incoming edges)
    pub fn dependencies_of(&self, idx: NodeIndex) -> Vec<NodeIndex> {
        self.graph
            .edges_directed(idx, Direction::Incoming)
            .map(|e| e.source())
            .collect()
    }

    /// Get the dependents of a specific task (outgoing edges)
    pub fn dependents_of(&self, idx: NodeIndex) -> Vec<NodeIndex> {
        self.graph
            .edges_directed(idx, Direction::Outgoing)
            .map(|e| e.target())
            .collect()
    }

    /// Assign an agent to a subtask
    pub fn assign_agent(&mut self, idx: NodeIndex, agent_id: impl Into<String>) {
        if let Some(task) = self.graph.node_weight_mut(idx) {
            task.assigned_agent = Some(agent_id.into());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_dag() {
        let dag = SubtaskDAG::new("empty");
        assert_eq!(dag.task_count(), 0);
        assert!(dag.is_complete());
        assert_eq!(dag.progress(), 1.0);
        assert!(dag.ready_tasks().is_empty());
    }

    #[test]
    fn test_single_task() {
        let mut dag = SubtaskDAG::new("single");
        let t1 = dag.add_task(SwarmSubtask::new("t1", "Task 1"));

        assert_eq!(dag.task_count(), 1);
        assert!(!dag.is_complete());

        // Single task with no deps should be ready
        let ready = dag.ready_tasks();
        assert_eq!(ready.len(), 1);
        assert_eq!(ready[0], t1);

        dag.mark_complete(t1);
        assert!(dag.is_complete());
        assert_eq!(dag.progress(), 1.0);
    }

    #[test]
    fn test_linear_chain() {
        let mut dag = SubtaskDAG::new("chain");
        let a = dag.add_task(SwarmSubtask::new("a", "Search"));
        let b = dag.add_task(SwarmSubtask::new("b", "Analyze"));
        let c = dag.add_task(SwarmSubtask::new("c", "Report"));

        dag.add_dependency(a, b).unwrap();
        dag.add_dependency(b, c).unwrap();

        // Only "a" is ready
        assert_eq!(dag.ready_tasks(), vec![a]);

        dag.mark_complete(a);
        assert_eq!(dag.ready_tasks(), vec![b]);

        dag.mark_complete(b);
        assert_eq!(dag.ready_tasks(), vec![c]);

        dag.mark_complete(c);
        assert!(dag.is_complete());
    }

    #[test]
    fn test_diamond_dag() {
        let mut dag = SubtaskDAG::new("diamond");
        let a = dag.add_task(SwarmSubtask::new("a", "Start"));
        let b = dag.add_task(SwarmSubtask::new("b", "Path 1"));
        let c = dag.add_task(SwarmSubtask::new("c", "Path 2"));
        let d = dag.add_task(SwarmSubtask::new("d", "Merge"));

        dag.add_dependency(a, b).unwrap();
        dag.add_dependency(a, c).unwrap();
        dag.add_dependency(b, d).unwrap();
        dag.add_dependency(c, d).unwrap();

        // only a has no dependencies
        let ready = dag.ready_tasks();
        assert_eq!(ready, vec![a]);

        dag.mark_complete(a);
        let mut ready = dag.ready_tasks();
        ready.sort();
        let mut expected = vec![b, c];
        expected.sort();
        assert_eq!(ready, expected); // b and c ready in parallel

        dag.mark_complete(b);
        // c is still pending; d must NOT be in ready list
        let ready_after_b = dag.ready_tasks();
        assert!(!ready_after_b.contains(&d), "d should not be ready while c is pending");

        dag.mark_complete(c);
        assert_eq!(dag.ready_tasks(), vec![d]); // now d is ready

        dag.mark_complete(d);
        assert!(dag.is_complete());
    }

    #[test]
    fn test_cycle_detection() {
        let mut dag = SubtaskDAG::new("cycle");
        let a = dag.add_task(SwarmSubtask::new("a", "A"));
        let b = dag.add_task(SwarmSubtask::new("b", "B"));

        dag.add_dependency(a, b).unwrap();
        let result = dag.add_dependency(b, a);

        assert!(result.is_err());
        // Edge should have been removed
        assert_eq!(dag.graph.edge_count(), 1);
    }

    #[test]
    fn test_topological_order() {
        let mut dag = SubtaskDAG::new("topo");
        let a = dag.add_task(SwarmSubtask::new("a", "A"));
        let b = dag.add_task(SwarmSubtask::new("b", "B"));
        let c = dag.add_task(SwarmSubtask::new("c", "C"));

        dag.add_dependency(a, b).unwrap();
        dag.add_dependency(b, c).unwrap();

        let order = dag.topological_order().unwrap();
        assert_eq!(order, vec![a, b, c]);
    }

    #[test]
    fn test_find_by_id() {
        let mut dag = SubtaskDAG::new("lookup");
        let a = dag.add_task(SwarmSubtask::new("search", "Search"));
        let _b = dag.add_task(SwarmSubtask::new("analyze", "Analyze"));

        assert_eq!(dag.find_by_id("search"), Some(a));
        assert_eq!(dag.find_by_id("nonexistent"), None);
    }

    #[test]
    fn test_soft_dependency() {
        let mut dag = SubtaskDAG::new("soft");
        let a = dag.add_task(SwarmSubtask::new("a", "Optional input"));
        let b = dag.add_task(SwarmSubtask::new("b", "Main task"));

        dag.add_dependency_with_kind(a, b, DependencyKind::Soft)
            .unwrap();

        // b should be ready even though a hasn't completed (soft dep)
        let ready = dag.ready_tasks();
        assert_eq!(ready.len(), 2); // both a and b ready
    }

    #[test]
    fn test_failed_task() {
        let mut dag = SubtaskDAG::new("failure");
        let a = dag.add_task(SwarmSubtask::new("a", "Will fail"));

        dag.mark_failed(a, "timeout");

        assert!(dag.is_complete()); // failed counts as terminal
        let task = dag.get_task(a).unwrap();
        assert!(matches!(task.status, SubtaskStatus::Failed(_)));
    }

    #[test]
    fn test_progress_tracking() {
        let mut dag = SubtaskDAG::new("progress");
        let a = dag.add_task(SwarmSubtask::new("a", "A"));
        let b = dag.add_task(SwarmSubtask::new("b", "B"));
        let c = dag.add_task(SwarmSubtask::new("c", "C"));
        let d = dag.add_task(SwarmSubtask::new("d", "D"));

        assert_eq!(dag.progress(), 0.0);

        dag.mark_complete(a);
        assert!((dag.progress() - 0.25).abs() < f64::EPSILON);

        dag.mark_complete(b);
        dag.mark_complete(c);
        dag.mark_complete(d);
        assert_eq!(dag.progress(), 1.0);

        let _ = (a, b, c, d);
    }

    #[test]
    fn test_agent_assignment() {
        let mut dag = SubtaskDAG::new("assign");
        let a = dag.add_task(SwarmSubtask::new("a", "A"));

        assert!(dag.get_task(a).unwrap().assigned_agent.is_none());

        dag.assign_agent(a, "agent-1");
        assert_eq!(
            dag.get_task(a).unwrap().assigned_agent.as_deref(),
            Some("agent-1")
        );
    }
}
