use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};

use crate::swarm::{AgentSpec, SubtaskDAG, SwarmSubtask};

#[derive(Debug, Default, Clone)]
pub struct SwarmCapabilityRegistry {
    agents: Vec<AgentSpec>,
    // capability name -> indices into `agents`
    index: HashMap<String, Vec<usize>>,
}

impl SwarmCapabilityRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(mut self, agent: AgentSpec) -> Self {
        let idx = self.agents.len();
        for cap in &agent.capabilities {
            self.index.entry(cap.clone()).or_default().push(idx);
        }
        self.agents.push(agent);
        self
    }

    pub fn find_by_capability(&self, cap: &str) -> Vec<&AgentSpec> {
        self.index
            .get(cap)
            .map(|idxs| idxs.iter().map(|&i| &self.agents[i]).collect())
            .unwrap_or_default()
    }

    // returns agents that satisfy ALL required capabilities of the task
    pub fn find_for_task(&self, task: &SwarmSubtask) -> Vec<&AgentSpec> {
        if task.required_capabilities.is_empty() {
            return self.agents.iter().collect();
        }
        self.agents
            .iter()
            .filter(|a| {
                task.required_capabilities
                    .iter()
                    .all(|req| a.capabilities.iter().any(|c| c == req))
            })
            .collect()
    }

    // pre-execution gap analysis: which tasks have no capable agent?
    pub fn coverage_report(&self, dag: &SubtaskDAG) -> CoverageReport {
        let mut covered = Vec::new();
        let mut uncovered = Vec::new();
        let mut partial = Vec::new();
        let mut gaps: HashSet<String> = HashSet::new();

        for (_, task) in dag.all_tasks() {
            if task.required_capabilities.is_empty() {
                covered.push(task.id.clone());
                continue;
            }

            for cap in &task.required_capabilities {
                if self.find_by_capability(cap).is_empty() {
                    gaps.insert(cap.clone());
                }
            }

            match self.find_for_task(task).len() {
                0 => uncovered.push(task.id.clone()),
                1 => partial.push(task.id.clone()),
                _ => covered.push(task.id.clone()),
            }
        }

        let mut gaps: Vec<String> = gaps.into_iter().collect();
        gaps.sort();

        CoverageReport { covered, uncovered, partial, gaps }
    }

    pub fn agents(&self) -> &[AgentSpec] {
        &self.agents
    }

    pub fn agent_count(&self) -> usize {
        self.agents.len()
    }

    pub fn capability_count(&self) -> usize {
        self.index.len()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoverageReport {
    /// task ids with 2 or more capable agents
    pub covered: Vec<String>,
    /// task ids with zero capable agents — will fail at dispatch
    pub uncovered: Vec<String>,
    /// task ids with exactly 1 capable agent — single point of failure
    pub partial: Vec<String>,
    /// capability names required by tasks but not registered by any agent
    pub gaps: Vec<String>,
}

impl CoverageReport {
    pub fn is_fully_covered(&self) -> bool {
        self.uncovered.is_empty()
    }

    pub fn has_spof_risk(&self) -> bool {
        !self.partial.is_empty()
    }

    pub fn problem_count(&self) -> usize {
        self.uncovered.len() + self.partial.len()
    }
}
