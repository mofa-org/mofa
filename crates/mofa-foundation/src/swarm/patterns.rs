//! Coordination Patterns for the Swarm Orchestrator
//!
//! Unifies all coordination strategies into a single enum that the
//! swarm orchestrator uses to select how agents collaborate.

use serde::{Deserialize, Serialize};

/// Coordination pattern for a swarm of agents
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CoordinationPattern {
    Sequential,
    Parallel,
    Debate,
    Consensus,
    MapReduce,
    Supervision,
    Routing,
}

impl Default for CoordinationPattern {
    fn default() -> Self {
        Self::Sequential
    }
}

impl std::fmt::Display for CoordinationPattern {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Sequential => write!(f, "Sequential"),
            Self::Parallel => write!(f, "Parallel"),
            Self::Debate => write!(f, "Debate"),
            Self::Consensus => write!(f, "Consensus"),
            Self::MapReduce => write!(f, "MapReduce"),
            Self::Supervision => write!(f, "Supervision"),
            Self::Routing => write!(f, "Routing"),
        }
    }
}

impl CoordinationPattern {
    /// Get a human-readable description of this pattern
    pub fn description(&self) -> &'static str {
        match self {
            Self::Sequential => "Agents execute subtasks one after another in dependency order",
            Self::Parallel => "Multiple agents execute independent subtasks simultaneously",
            Self::Debate => "Agents argue opposing positions, a judge synthesizes a conclusion",
            Self::Consensus => "Agents propose and iteratively converge on agreement",
            Self::MapReduce => "Input split into chunks, processed in parallel, results aggregated",
            Self::Supervision => "Supervisor monitors workers, reassigns failures, escalates to HITL",
            Self::Routing => "Router dispatches task to best matching specialist agent",
        }
    }

    /// Minimum number of agents required for this pattern
    pub fn min_agents(&self) -> usize {
        match self {
            Self::Sequential => 1,
            Self::Parallel => 2,
            Self::Debate => 3,
            Self::Consensus => 2,
            Self::MapReduce => 2,
            Self::Supervision => 2,
            Self::Routing => 2,
        }
    }

    /// Whether this pattern requires a designated leader/coordinator agent
    pub fn requires_leader(&self) -> bool {
        matches!(self, Self::Debate | Self::Supervision | Self::Routing)
    }

    /// All available patterns
    pub fn all() -> Vec<Self> {
        vec![
            Self::Sequential,
            Self::Parallel,
            Self::Debate,
            Self::Consensus,
            Self::MapReduce,
            Self::Supervision,
            Self::Routing,
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_serde_roundtrip() {
        for pattern in CoordinationPattern::all() {
            let json = serde_json::to_string(&pattern).unwrap();
            let deserialized: CoordinationPattern = serde_json::from_str(&json).unwrap();
            assert_eq!(pattern, deserialized);
        }
    }

    #[test]
    fn test_yaml_parse() {
        let yaml = "\"debate\"";
        let pattern: CoordinationPattern = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(pattern, CoordinationPattern::Debate);
    }

    #[test]
    fn test_min_agents() {
        assert_eq!(CoordinationPattern::Sequential.min_agents(), 1);
        assert_eq!(CoordinationPattern::Debate.min_agents(), 3);
        assert_eq!(CoordinationPattern::MapReduce.min_agents(), 2);
    }

    #[test]
    fn test_requires_leader() {
        assert!(!CoordinationPattern::Sequential.requires_leader());
        assert!(CoordinationPattern::Debate.requires_leader());
        assert!(CoordinationPattern::Supervision.requires_leader());
    }
}
