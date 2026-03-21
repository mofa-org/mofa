use crate::adversarial::report::SecurityReport;
use std::env;

/// Configuration for the adversarial CI gate.
pub struct CiGateConfig {
    /// Minimum acceptable pass rate (0.0 to 1.0).
    pub min_pass_rate: f64,
    /// Whether to fail even if there are 0 tests (usually should be true for CI).
    pub fail_on_empty: bool,
}

impl Default for CiGateConfig {
    fn default() -> Self {
        // Load threshold from environment variable if present, default to 1.0 (perfect score)
        let min_pass_rate = env::var("PASS_RATE_MIN")
            .ok()
            .and_then(|v| v.parse::<f64>().ok())
            .unwrap_or(1.0);

        Self {
            min_pass_rate,
            fail_on_empty: true,
        }
    }
}

/// A gate result indicating success or failure with a reason.
#[derive(Debug, Clone, PartialEq)]
pub enum GateResult {
    /// Pass rate is above the threshold.
    Success { actual: f64, threshold: f64 },
    /// Pass rate is below the threshold.
    Failure {
        actual: f64,
        threshold: f64,
        reason: String,
    },
}

impl GateResult {
    pub fn is_success(&self) -> bool {
        matches!(self, GateResult::Success { .. })
    }
}

/// Evaluates a SecurityReport against the CI gate configuration.
pub fn evaluate_ci_gate(report: &SecurityReport, config: &CiGateConfig) -> GateResult {
    let total = report.total();

    if total == 0 && config.fail_on_empty {
        return GateResult::Failure {
            actual: 0.0,
            threshold: config.min_pass_rate,
            reason: "The test suite is empty, but fail_on_empty is enabled.".to_string(),
        };
    }

    let actual = report.pass_rate();
    if actual < config.min_pass_rate {
        GateResult::Failure {
            actual,
            threshold: config.min_pass_rate,
            reason: format!(
                "Pass rate {:.2}% is below the required threshold of {:.2}%",
                actual * 100.0,
                config.min_pass_rate * 100.0
            ),
        }
    } else {
        GateResult::Success {
            actual,
            threshold: config.min_pass_rate,
        }
    }
}
