//! Swarm Composer
//!
//! Deterministic capability-aware subtask assignment bridge.

use std::collections::{HashMap, HashSet};

use petgraph::graph::NodeIndex;
use serde::{Deserialize, Serialize};

use crate::CapabilityRegistry;

use super::config::{AgentSpec, AuditEvent, AuditEventKind};
use super::dag::SubtaskDAG;

/// Fallback strategy when no exact capability match is found.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FallbackPolicy {
    /// Do not assign the subtask if no exact capability match exists.
    None,
    /// Use capability-registry semantic query as a secondary candidate source.
    RegistryQuery,
    /// Assign to any available agent using deterministic load-balanced tie-breaking.
    AnyAgent,
}

/// Result of one composition pass.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SwarmComposeResult {
    pub assigned: usize,
    pub unassigned: Vec<String>,
    pub audit_events: Vec<AuditEvent>,
}

/// Capability-aware assignment engine for swarm subtasks.
pub struct SwarmComposer {
    agents: Vec<AgentSpec>,
    registry: CapabilityRegistry,
    fallback_policy: FallbackPolicy,
}

impl SwarmComposer {
    /// Build a composer from static swarm agent specs.
    pub fn new(agents: Vec<AgentSpec>) -> Self {
        let mut registry = CapabilityRegistry::new();
        for agent in &agents {
            registry.register(agent.to_manifest());
        }
        Self {
            agents,
            registry,
            fallback_policy: FallbackPolicy::RegistryQuery,
        }
    }

    /// Set fallback policy.
    pub fn with_fallback_policy(mut self, policy: FallbackPolicy) -> Self {
        self.fallback_policy = policy;
        self
    }

    /// Assign agents to all currently-unassigned subtasks in the DAG.
    ///
    /// Determinism guarantees:
    /// - tasks visited in sorted `subtask_id` order
    /// - candidate tie-break by current assigned-load, then `agent_id` lexicographically
    pub fn compose_assignments(&self, dag: &mut SubtaskDAG) -> SwarmComposeResult {
        let mut result = SwarmComposeResult::default();
        let mut load = self.initial_load_from_dag(dag);

        let mut tasks: Vec<(NodeIndex, String)> = dag
            .unassigned_tasks()
            .into_iter()
            .filter_map(|idx| dag.get_task(idx).map(|t| (idx, t.id.clone())))
            .collect();
        tasks.sort_by(|a, b| a.1.cmp(&b.1));

        for (idx, task_id) in tasks {
            let Some(task) = dag.get_task(idx).cloned() else {
                continue;
            };

            match self.pick_agent_for_subtask(&task.required_capabilities, &task.description, &load)
            {
                Some(agent_id) => {
                    dag.assign_agent(idx, agent_id.clone());
                    *load.entry(agent_id.clone()).or_insert(0) += 1;
                    result.assigned += 1;
                    result.audit_events.push(
                        AuditEvent::new(
                            AuditEventKind::AgentAssigned,
                            format!("Assigned agent '{}' to subtask '{}'", agent_id, task_id),
                        )
                        .with_data(serde_json::json!({
                            "subtask_id": task_id,
                            "agent_id": agent_id,
                        })),
                    );
                }
                None => {
                    result.unassigned.push(task_id.clone());
                    result.audit_events.push(
                        AuditEvent::new(
                            AuditEventKind::SubtaskFailed,
                            format!("No capable agent found for subtask '{}'", task_id),
                        )
                        .with_data(serde_json::json!({
                            "subtask_id": task_id,
                            "required_capabilities": task.required_capabilities,
                        })),
                    );
                }
            }
        }

        result
    }

    fn initial_load_from_dag(&self, dag: &SubtaskDAG) -> HashMap<String, usize> {
        let mut load = HashMap::new();
        for (_, task) in dag.all_tasks() {
            if let Some(agent_id) = &task.assigned_agent {
                *load.entry(agent_id.clone()).or_insert(0) += 1;
            }
        }
        load
    }

    fn pick_agent_for_subtask(
        &self,
        required_capabilities: &[String],
        description: &str,
        load: &HashMap<String, usize>,
    ) -> Option<String> {
        if self.agents.is_empty() {
            return None;
        }

        let required = normalize_caps(required_capabilities);

        // 1) Exact capability match: required caps are a subset of agent caps.
        let exact_candidates: Vec<String> = self
            .agents
            .iter()
            .filter(|a| required.is_subset(&normalize_caps(&a.capabilities)))
            .map(|a| a.id.clone())
            .collect();

        if let Some(agent_id) = self.pick_least_loaded(exact_candidates, load) {
            return Some(agent_id);
        }

        // 2) Fallback policies.
        match self.fallback_policy {
            FallbackPolicy::None => None,
            FallbackPolicy::RegistryQuery => {
                let query = format!("{} {}", description, required_capabilities.join(" "));
                let candidates: Vec<String> = self
                    .registry
                    .query(&query)
                    .into_iter()
                    .map(|m| m.agent_id.clone())
                    .collect();
                self.pick_least_loaded(candidates, load)
            }
            FallbackPolicy::AnyAgent => {
                let candidates = self.agents.iter().map(|a| a.id.clone()).collect();
                self.pick_least_loaded(candidates, load)
            }
        }
    }

    fn pick_least_loaded(
        &self,
        candidates: Vec<String>,
        load: &HashMap<String, usize>,
    ) -> Option<String> {
        if candidates.is_empty() {
            return None;
        }
        let known: HashSet<&str> = self.agents.iter().map(|a| a.id.as_str()).collect();

        candidates
            .into_iter()
            .filter(|id| known.contains(id.as_str()))
            .min_by(|a, b| {
                let la = load.get(a).copied().unwrap_or(0);
                let lb = load.get(b).copied().unwrap_or(0);
                la.cmp(&lb).then_with(|| a.cmp(b))
            })
    }
}

fn normalize_caps(caps: &[String]) -> HashSet<String> {
    caps.iter()
        .map(|c| c.trim().to_ascii_lowercase())
        .filter(|c| !c.is_empty())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::swarm::dag::SwarmSubtask;

    fn agent(id: &str, caps: &[&str]) -> AgentSpec {
        AgentSpec {
            id: id.to_string(),
            capabilities: caps.iter().map(|s| s.to_string()).collect(),
            model: None,
            cost_per_token: None,
            max_concurrency: 1,
        }
    }

    #[test]
    fn test_exact_match_assignment() {
        let mut dag = SubtaskDAG::new("exact");
        let idx = dag.add_task(
            SwarmSubtask::new("task-1", "Analyze data")
                .with_capabilities(vec!["analysis".to_string(), "sql".to_string()]),
        );

        let composer = SwarmComposer::new(vec![
            agent("agent-a", &["analysis"]),
            agent("agent-b", &["analysis", "sql"]),
        ]);
        let out = composer.compose_assignments(&mut dag);

        assert_eq!(out.assigned, 1);
        assert!(out.unassigned.is_empty());
        assert_eq!(
            dag.get_task(idx).and_then(|t| t.assigned_agent.clone()),
            Some("agent-b".to_string())
        );
        assert!(
            out.audit_events
                .iter()
                .any(|e| matches!(e.kind, AuditEventKind::AgentAssigned))
        );
    }

    #[test]
    fn test_registry_fallback_assignment() {
        let mut dag = SubtaskDAG::new("fallback");
        let idx = dag.add_task(
            SwarmSubtask::new("task-1", "Write rust code")
                .with_capabilities(vec!["missing-required-cap".to_string()]),
        );

        let composer = SwarmComposer::new(vec![
            agent("agent-rust", &["rust", "coding"]),
            agent("agent-docs", &["docs", "writing"]),
        ])
        .with_fallback_policy(FallbackPolicy::RegistryQuery);

        let out = composer.compose_assignments(&mut dag);
        assert_eq!(out.assigned, 1);
        assert_eq!(
            dag.get_task(idx).and_then(|t| t.assigned_agent.clone()),
            Some("agent-rust".to_string())
        );
    }

    #[test]
    fn test_no_match_without_fallback() {
        let mut dag = SubtaskDAG::new("no-match");
        let idx = dag.add_task(
            SwarmSubtask::new("task-1", "Unknown task")
                .with_capabilities(vec!["nonexistent-cap".to_string()]),
        );

        let composer = SwarmComposer::new(vec![agent("agent-a", &["analysis"])])
            .with_fallback_policy(FallbackPolicy::None);
        let out = composer.compose_assignments(&mut dag);

        assert_eq!(out.assigned, 0);
        assert_eq!(out.unassigned, vec!["task-1".to_string()]);
        assert!(dag.get_task(idx).unwrap().assigned_agent.is_none());
        assert!(
            out.audit_events
                .iter()
                .any(|e| matches!(e.kind, AuditEventKind::SubtaskFailed))
        );
    }
}
