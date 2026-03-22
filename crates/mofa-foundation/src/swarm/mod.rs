//! Swarm Orchestrator Module

pub mod analyzer;
pub mod config;
pub mod dag;
pub mod hitl;
pub mod patterns;
pub mod scheduler;
pub mod telemetry;

pub use analyzer::{RiskAwareAnalysis, RiskSummary, TaskAnalyzer};
pub use config::{
    AgentSpec, AuditEvent, AuditEventKind, HITLMode, SLAConfig, SwarmConfig, SwarmMetrics,
    SwarmResult, SwarmStatus,
};
pub use hitl::{
    ApprovalDecision, ApprovalHandler, ApprovalOutcome, ApprovalRequest,
    ChannelApprovalHandler, hitl_executor_middleware,
};
pub use dag::{DependencyEdge, DependencyKind, RiskLevel, SubtaskDAG, SubtaskStatus, SwarmSubtask};
pub use patterns::CoordinationPattern;
pub use scheduler::{
    FailurePolicy, ParallelScheduler, SchedulerSummary, SequentialScheduler, SubtaskExecutorFn,
    SwarmScheduler, SwarmSchedulerConfig, TaskExecutionResult, TaskOutcome,
};
pub use telemetry::{audit_batch_to_debug, audit_to_debug};
