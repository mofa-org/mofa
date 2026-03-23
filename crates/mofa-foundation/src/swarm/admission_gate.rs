use std::collections::HashSet;
use std::sync::Arc;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::swarm::{RiskLevel, SubtaskDAG, SwarmSubtask};

// ── Verdict types ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub enum PolicyVerdict {
    Allow,
    Warn(String),
    Deny(String),
}

impl PolicyVerdict {
    pub fn is_denial(&self) -> bool {
        matches!(self, Self::Deny(_))
    }

    pub fn is_warning(&self) -> bool {
        matches!(self, Self::Warn(_))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskVerdict {
    pub task_id: String,
    pub policy: String,
    pub verdict: PolicyVerdict,
}

// ── AdmissionDecision ──────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub enum AdmissionDecision {
    Allowed,
    AllowedWithWarnings(Vec<String>),
    Denied(Vec<String>),
}

impl AdmissionDecision {
    pub fn is_allowed(&self) -> bool {
        !matches!(self, Self::Denied(_))
    }

    pub fn is_denied(&self) -> bool {
        matches!(self, Self::Denied(_))
    }
}

// ── AdmissionReport ────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdmissionReport {
    pub decision: AdmissionDecision,
    pub task_verdicts: Vec<TaskVerdict>,
    pub evaluated_at: DateTime<Utc>,
}

impl AdmissionReport {
    pub fn is_allowed(&self) -> bool {
        self.decision.is_allowed()
    }
}

// ── AdmissionPolicy trait ──────────────────────────────────────────────────────

pub trait AdmissionPolicy: Send + Sync {
    fn name(&self) -> &str;

    // dag-level check; override for aggregate rules
    fn evaluate_dag(&self, _dag: &SubtaskDAG) -> PolicyVerdict {
        PolicyVerdict::Allow
    }

    // per-task check; override for per-task rules
    fn evaluate_task(&self, _task: &SwarmSubtask) -> PolicyVerdict {
        PolicyVerdict::Allow
    }
}

// ── SwarmAdmissionGate ─────────────────────────────────────────────────────────

#[derive(Default)]
pub struct SwarmAdmissionGate {
    policies: Vec<Arc<dyn AdmissionPolicy>>,
}

impl SwarmAdmissionGate {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_policy(mut self, policy: impl AdmissionPolicy + 'static) -> Self {
        self.policies.push(Arc::new(policy));
        self
    }

    pub fn evaluate(&self, dag: &SubtaskDAG) -> AdmissionReport {
        let tasks = dag.all_tasks();
        let mut task_verdicts: Vec<TaskVerdict> = Vec::new();
        let mut denials: Vec<String> = Vec::new();
        let mut warnings: Vec<String> = Vec::new();

        for policy in &self.policies {
            match policy.evaluate_dag(dag) {
                PolicyVerdict::Deny(r) => denials.push(r),
                PolicyVerdict::Warn(w) => warnings.push(w),
                PolicyVerdict::Allow => {}
            }

            for (_, task) in &tasks {
                match policy.evaluate_task(task) {
                    PolicyVerdict::Allow => {}
                    PolicyVerdict::Deny(ref r) => {
                        denials.push(format!("task {}: {}", task.id, r));
                        task_verdicts.push(TaskVerdict {
                            task_id: task.id.clone(),
                            policy: policy.name().to_string(),
                            verdict: PolicyVerdict::Deny(r.clone()),
                        });
                    }
                    PolicyVerdict::Warn(ref w) => {
                        warnings.push(format!("task {}: {}", task.id, w));
                        task_verdicts.push(TaskVerdict {
                            task_id: task.id.clone(),
                            policy: policy.name().to_string(),
                            verdict: PolicyVerdict::Warn(w.clone()),
                        });
                    }
                }
            }
        }

        let decision = if !denials.is_empty() {
            AdmissionDecision::Denied(denials)
        } else if !warnings.is_empty() {
            AdmissionDecision::AllowedWithWarnings(warnings)
        } else {
            AdmissionDecision::Allowed
        };

        AdmissionReport {
            decision,
            task_verdicts,
            evaluated_at: Utc::now(),
        }
    }
}

// ── Built-in policies ──────────────────────────────────────────────────────────

/// denies dags that exceed a maximum task count
pub struct MaxTaskCountPolicy {
    pub limit: usize,
}

impl AdmissionPolicy for MaxTaskCountPolicy {
    fn name(&self) -> &str {
        "max_task_count"
    }

    fn evaluate_dag(&self, dag: &SubtaskDAG) -> PolicyVerdict {
        if dag.task_count() > self.limit {
            PolicyVerdict::Deny(format!(
                "dag has {} tasks, limit is {}",
                dag.task_count(),
                self.limit
            ))
        } else {
            PolicyVerdict::Allow
        }
    }
}

/// denies dags that exceed configured counts of high/critical tasks
pub struct RiskBudgetPolicy {
    pub max_critical: usize,
    pub max_high: usize,
}

impl AdmissionPolicy for RiskBudgetPolicy {
    fn name(&self) -> &str {
        "risk_budget"
    }

    fn evaluate_dag(&self, dag: &SubtaskDAG) -> PolicyVerdict {
        let mut critical = 0usize;
        let mut high = 0usize;

        for (_, task) in dag.all_tasks() {
            match task.risk_level {
                RiskLevel::Critical => critical += 1,
                RiskLevel::High => high += 1,
                _ => {}
            }
        }

        if critical > self.max_critical {
            return PolicyVerdict::Deny(format!(
                "{critical} critical tasks exceed limit of {}",
                self.max_critical
            ));
        }
        if high > self.max_high {
            return PolicyVerdict::Deny(format!(
                "{high} high-risk tasks exceed limit of {}",
                self.max_high
            ));
        }
        PolicyVerdict::Allow
    }
}

/// denies tasks that require capabilities not in the allowed set
pub struct RequiredCapabilityPolicy {
    pub allowed: HashSet<String>,
}

impl RequiredCapabilityPolicy {
    pub fn new(caps: impl IntoIterator<Item = impl Into<String>>) -> Self {
        Self {
            allowed: caps.into_iter().map(|s| s.into()).collect(),
        }
    }
}

impl AdmissionPolicy for RequiredCapabilityPolicy {
    fn name(&self) -> &str {
        "required_capability"
    }

    fn evaluate_task(&self, task: &SwarmSubtask) -> PolicyVerdict {
        for cap in &task.required_capabilities {
            if !self.allowed.contains(cap) {
                return PolicyVerdict::Deny(format!("unknown capability \"{cap}\""));
            }
        }
        PolicyVerdict::Allow
    }
}

/// warns when total dag complexity exceeds a budget
pub struct ComplexityBudgetPolicy {
    pub max_total: f64,
}

impl AdmissionPolicy for ComplexityBudgetPolicy {
    fn name(&self) -> &str {
        "complexity_budget"
    }

    fn evaluate_dag(&self, dag: &SubtaskDAG) -> PolicyVerdict {
        let total: f64 = dag.all_tasks().iter().map(|(_, t)| t.complexity).sum();
        if total > self.max_total {
            PolicyVerdict::Warn(format!(
                "total complexity {total:.2} exceeds budget {:.2}",
                self.max_total
            ))
        } else {
            PolicyVerdict::Allow
        }
    }
}
