//! Swarm Orchestrator Module

pub mod config;
pub mod dag;
pub mod patterns;
pub mod hitl;

// Re-export core types
pub use config::{
    AgentSpec, AuditEvent, AuditEventKind, HITLMode, SLAConfig, SwarmConfig, SwarmMetrics,
    SwarmResult, SwarmStatus,
};
pub use dag::{DependencyEdge, DependencyKind, SwarmSubtask, SubtaskDAG, SubtaskStatus};
pub use patterns::CoordinationPattern;
pub use hitl::{
    ApprovalDecision, ApprovalHandler, ApprovalOutcome, ApprovalRequest,
    ChannelApprovalHandler, ExecutionResult, SubtaskOutput, run_sequential_with_hitl,
};
