use crate::adversarial::policy::PolicyOutcome;
use crate::adversarial::suite::AdversarialCategory;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SecurityCaseResult {
    pub case_id: String,
    pub category: AdversarialCategory,
    pub outcome: PolicyOutcome,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SecurityReport {
    pub results: Vec<SecurityCaseResult>,
}

impl SecurityReport {
    pub fn total(&self) -> usize {
        self.results.len()
    }

    pub fn passed(&self) -> usize {
        self.results
            .iter()
            .filter(|r| matches!(r.outcome, PolicyOutcome::Pass))
            .count()
    }

    pub fn failed(&self) -> usize {
        self.results
            .iter()
            .filter(|r| matches!(r.outcome, PolicyOutcome::Fail { .. }))
            .count()
    }

    pub fn pass_rate(&self) -> f64 {
        let total = self.total();
        if total == 0 {
            return 1.0;
        }
        self.passed() as f64 / total as f64
    }

    pub fn failures(&self) -> impl Iterator<Item = &SecurityCaseResult> {
        self.results
            .iter()
            .filter(|r| matches!(r.outcome, PolicyOutcome::Fail { .. }))
    }
}
