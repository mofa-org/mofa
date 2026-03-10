//! Core data types for test reports.

use std::time::Duration;

/// Outcome of a single test case.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum TestStatus {
    Passed,
    Failed,
    Skipped,
}

impl std::fmt::Display for TestStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Passed => write!(f, "passed"),
            Self::Failed => write!(f, "failed"),
            Self::Skipped => write!(f, "skipped"),
        }
    }
}

/// Result of a single test case execution.
#[derive(Debug, Clone)]
pub struct TestCaseResult {
    pub name: String,
    pub status: TestStatus,
    pub duration: Duration,
    pub error: Option<String>,
    pub metadata: Vec<(String, String)>,
}

/// Aggregated report for a test suite.
#[derive(Debug, Clone)]
pub struct TestReport {
    pub suite_name: String,
    pub results: Vec<TestCaseResult>,
    pub total_duration: Duration,
    pub timestamp: u64,
}

impl TestReport {
    /// Total number of test cases.
    pub fn total(&self) -> usize {
        self.results.len()
    }

    /// Number of passed test cases.
    pub fn passed(&self) -> usize {
        self.results
            .iter()
            .filter(|r| r.status == TestStatus::Passed)
            .count()
    }

    /// Number of failed test cases.
    pub fn failed(&self) -> usize {
        self.results
            .iter()
            .filter(|r| r.status == TestStatus::Failed)
            .count()
    }

    /// Number of skipped test cases.
    pub fn skipped(&self) -> usize {
        self.results
            .iter()
            .filter(|r| r.status == TestStatus::Skipped)
            .count()
    }

    /// Pass rate as a fraction in `[0.0, 1.0]`. Returns `1.0` for an empty suite.
    pub fn pass_rate(&self) -> f64 {
        let total = self.total();
        if total == 0 {
            return 1.0;
        }
        self.passed() as f64 / total as f64
    }

    /// The `n` slowest test cases, sorted by descending duration.
    pub fn slowest(&self, n: usize) -> Vec<&TestCaseResult> {
        let mut sorted: Vec<&TestCaseResult> = self.results.iter().collect();
        sorted.sort_by(|a, b| b.duration.cmp(&a.duration));
        sorted.truncate(n);
        sorted
    }

    /// All test cases that failed.
    pub fn failures(&self) -> Vec<&TestCaseResult> {
        self.results
            .iter()
            .filter(|r| r.status == TestStatus::Failed)
            .collect()
    }
}
