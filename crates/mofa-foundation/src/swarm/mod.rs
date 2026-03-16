//! Swarm Orchestrator Module

pub mod analyzer;
pub mod config;
pub mod dag;
pub mod patterns;

// Re-export core types
pub use analyzer::TaskAnalyzer;
pub use config::{
    AgentSpec, AuditEvent, AuditEventKind, HITLMode, SLAConfig, SwarmConfig, SwarmMetrics,
    SwarmResult, SwarmStatus,
};
pub use dag::{DependencyEdge, DependencyKind, SwarmSubtask, SubtaskDAG, SubtaskStatus};
pub use patterns::CoordinationPattern;
