//! Swarm Orchestrator Module

pub mod analyzer;
pub mod config;
pub mod dag;
pub mod patterns;
pub mod executor;

pub mod telemetry;

// Re-export core types
pub use analyzer::TaskAnalyzer;
pub use config::{
    AgentSpec, AuditEvent, AuditEventKind, HITLMode, SLAConfig, SwarmConfig, SwarmMetrics,
    SwarmResult, SwarmStatus,
};
pub use dag::{DependencyEdge, DependencyKind, SwarmSubtask, SubtaskDAG, SubtaskStatus};
pub use patterns::CoordinationPattern;
pub use executor::{run_sequential, run_parallel, ExecutionResult, SubtaskOutput};
pub use telemetry::{audit_to_debug, audit_batch_to_debug};

