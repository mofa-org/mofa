//! Swarm Configuration and Result types

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

use super::dag::SubtaskDAG;
use super::patterns::CoordinationPattern;

// Swarm Configuration
/// Top-level swarm orchestration configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SwarmConfig {
    /// Unique identifier (auto-generated if not specified)
    #[serde(default = "default_id")]
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub description: String,
    pub task: String,
    #[serde(default)]
    pub agents: Vec<AgentSpec>,
    #[serde(default)]
    pub pattern: CoordinationPattern,
    #[serde(default)]
    pub sla: SLAConfig,
    #[serde(default)]
    pub hitl: HITLMode,
    #[serde(default)]
    pub metadata: HashMap<String, String>,
}

fn default_id() -> String {
    Uuid::now_v7().to_string()
}

/// Agent specification in swarm config
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentSpec {
    pub id: String,
    #[serde(default)]
    pub capabilities: Vec<String>,
    pub model: Option<String>,
    pub cost_per_token: Option<f64>,
    #[serde(default = "default_concurrency")]
    pub max_concurrency: u32,
}

fn default_concurrency() -> u32 {
    1
}

/// SLA constraints for a swarm run
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SLAConfig {
    #[serde(default)]
    pub max_duration_secs: u64,
    #[serde(default)]
    pub max_cost_tokens: u64,
    #[serde(default)]
    pub min_quality: f64,
}

impl Default for SLAConfig {
    fn default() -> Self {
        Self {
            max_duration_secs: 0,
            max_cost_tokens: 0,
            min_quality: 0.0,
        }
    }
}

/// Human-in-the-loop mode
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum HITLMode {
    None,
    Required,
    #[default]
    Optional,
}

// Swarm Result
/// Result of a swarm orchestration run
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SwarmResult {
    pub config_id: String,
    pub status: SwarmStatus,
    pub dag: SubtaskDAG,
    pub output: Option<String>,
    pub metrics: SwarmMetrics,
    pub audit_events: Vec<AuditEvent>,
    pub started_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
}

/// Overall swarm execution status
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum SwarmStatus {
    Running,
    Completed,
    Failed(String),
    Cancelled,
    Escalated,
}

/// Execution metrics for a swarm run
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SwarmMetrics {
    pub total_tokens: u64,
    pub duration_ms: u64,
    pub tasks_completed: usize,
    pub tasks_failed: usize,
    pub hitl_interventions: usize,
    pub reassignments: usize,
    pub agent_tokens: HashMap<String, u64>,
}

impl SwarmMetrics {
    pub fn record_task_completed(&mut self) {
        self.tasks_completed += 1;
    }

    pub fn record_task_failed(&mut self) {
        self.tasks_failed += 1;
    }

    pub fn record_hitl_intervention(&mut self) {
        self.hitl_interventions += 1;
    }

    pub fn record_reassignment(&mut self) {
        self.reassignments += 1;
    }

    pub fn add_tokens(&mut self, tokens: u64) {
        self.total_tokens += tokens;
    }

    // Aggregate token usage both globally and per agent.
    pub fn record_agent_tokens(&mut self, agent_id: impl Into<String>, tokens: u64) {
        self.total_tokens += tokens;
        *self.agent_tokens.entry(agent_id.into()).or_insert(0) += tokens;
    }

    pub fn set_duration_ms(&mut self, duration_ms: u64) {
        self.duration_ms = duration_ms;
    }
}

/// An audit event logged during swarm execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditEvent {
    pub timestamp: DateTime<Utc>,
    pub kind: AuditEventKind,
    pub description: String,
    #[serde(default)]
    pub data: serde_json::Value,
}

/// Types of audit events
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum AuditEventKind {
    SwarmStarted,
    TaskDecomposed,
    AgentAssigned,
    PatternSelected,
    SubtaskStarted,
    SubtaskCompleted,
    SubtaskFailed,
    HITLRequested,
    HITLDecision,
    SLAWarning,
    SLABreach,
    AgentReassigned,
    SwarmCompleted,
    // admission gate outcomes
    AdmissionChecked,
    AdmissionDenied,
    // scheduler lifecycle
    SchedulerStarted,
    SchedulerCompleted,
}

impl AuditEvent {
    /// Create a new audit event
    pub fn new(kind: AuditEventKind, description: impl Into<String>) -> Self {
        Self {
            timestamp: Utc::now(),
            kind,
            description: description.into(),
            data: serde_json::Value::Null,
        }
    }

    /// Attach data to the event
    pub fn with_data(mut self, data: serde_json::Value) -> Self {
        self.data = data;
        self
    }

    pub fn record_into(self, log: &mut Vec<AuditEvent>) {
        log.push(self);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_yaml_parse() {
        let yaml = r#"
name: test-swarm
description: A test swarm
task: "Research and summarize AI trends"
agents:
  - id: researcher
    capabilities: [web_search, summarize]
  - id: analyst
    capabilities: [analyze, compare]
pattern: debate
sla:
  max_duration_secs: 120
  max_cost_tokens: 5000
hitl: required
"#;

        let config: SwarmConfig = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(config.name, "test-swarm");
        assert_eq!(config.agents.len(), 2);
        assert_eq!(config.agents[0].id, "researcher");
        assert_eq!(config.pattern, CoordinationPattern::Debate);
        assert_eq!(config.sla.max_duration_secs, 120);
        assert_eq!(config.hitl, HITLMode::Required);
    }

    #[test]
    fn test_config_defaults() {
        let yaml = r#"
name: minimal
task: "Do something"
"#;

        let config: SwarmConfig = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(config.pattern, CoordinationPattern::Sequential);
        assert_eq!(config.hitl, HITLMode::Optional);
        assert_eq!(config.sla.max_duration_secs, 0);
    }

    #[test]
    fn test_audit_event_creation() {
        let event = AuditEvent::new(AuditEventKind::SwarmStarted, "Swarm started with 3 agents")
            .with_data(serde_json::json!({"agent_count": 3}));

        assert_eq!(event.kind, AuditEventKind::SwarmStarted);
        assert_eq!(event.data["agent_count"], 3);
    }

    #[test]
    fn test_swarm_metrics_helpers() {
        let mut metrics = SwarmMetrics::default();

        metrics.record_task_completed();
        metrics.record_task_failed();
        metrics.record_hitl_intervention();
        metrics.record_reassignment();
        metrics.add_tokens(25);
        metrics.record_agent_tokens("agent-a", 15);
        metrics.set_duration_ms(42);

        assert_eq!(metrics.tasks_completed, 1);
        assert_eq!(metrics.tasks_failed, 1);
        assert_eq!(metrics.hitl_interventions, 1);
        assert_eq!(metrics.reassignments, 1);
        assert_eq!(metrics.total_tokens, 40);
        assert_eq!(metrics.agent_tokens.get("agent-a"), Some(&15));
        assert_eq!(metrics.duration_ms, 42);
    }

    #[test]
    fn test_audit_event_record_into_log() {
        let mut log = Vec::new();

        AuditEvent::new(AuditEventKind::SubtaskCompleted, "subtask finished").record_into(&mut log);

        assert_eq!(log.len(), 1);
        assert_eq!(log[0].kind, AuditEventKind::SubtaskCompleted);
        assert_eq!(log[0].description, "subtask finished");
    }
}
