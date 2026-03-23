//! Core data types for test reports.

use std::time::Duration;

pub(crate) const OUTPUT_METADATA_KEY: &str = "output";
pub(crate) const TOOL_CALLS_METADATA_KEY: &str = "tool_calls";
pub(crate) const RETRY_COUNT_METADATA_KEY: &str = "retry_count";
pub(crate) const FALLBACK_TRIGGERED_METADATA_KEY: &str = "fallback_triggered";

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

impl TestCaseResult {
    /// Attach metadata entries to this result.
    pub fn with_metadata<I, K, V>(mut self, metadata: I) -> Self
    where
        I: IntoIterator<Item = (K, V)>,
        K: Into<String>,
        V: Into<String>,
    {
        self.metadata
            .extend(metadata.into_iter().map(|(key, value)| (key.into(), value.into())));
        self
    }

    /// Look up a metadata value by key.
    pub fn metadata_value(&self, key: &str) -> Option<&str> {
        self.metadata
            .iter()
            .find(|(existing, _)| existing == key)
            .map(|(_, value)| value.as_str())
    }

    /// Look up the first present metadata value across multiple key aliases.
    pub fn metadata_value_any<'a>(&'a self, keys: &[&str]) -> Option<&'a str> {
        keys.iter().find_map(|key| self.metadata_value(key))
    }

    /// Attach a canonical output value used by higher-level diffing.
    pub fn with_output(self, output: impl Into<String>) -> Self {
        self.with_metadata([(OUTPUT_METADATA_KEY, output.into())])
    }

    /// Attach canonical tool-call trace text used by higher-level diffing.
    pub fn with_tool_calls(self, tool_calls: impl Into<String>) -> Self {
        self.with_metadata([(TOOL_CALLS_METADATA_KEY, tool_calls.into())])
    }

    /// Attach a canonical retry count used by higher-level diffing.
    pub fn with_retry_count(self, retry_count: usize) -> Self {
        self.with_metadata([(RETRY_COUNT_METADATA_KEY, retry_count.to_string())])
    }

    /// Attach canonical fallback status used by higher-level diffing.
    pub fn with_fallback_triggered(self, fallback_triggered: bool) -> Self {
        self.with_metadata([(
            FALLBACK_TRIGGERED_METADATA_KEY,
            fallback_triggered.to_string(),
        )])
    }

    /// Read the canonical output value if present.
    pub fn output(&self) -> Option<&str> {
        self.metadata_value(OUTPUT_METADATA_KEY)
    }

    /// Read the canonical tool-call trace if present.
    pub fn tool_calls(&self) -> Option<&str> {
        self.metadata_value(TOOL_CALLS_METADATA_KEY)
    }

    /// Read the canonical retry count if present and parseable.
    pub fn retry_count(&self) -> Option<usize> {
        self.metadata_value(RETRY_COUNT_METADATA_KEY)
            .and_then(|value| value.parse::<usize>().ok())
    }

    /// Read the canonical fallback status if present.
    pub fn fallback_triggered(&self) -> Option<bool> {
        self.metadata_value(FALLBACK_TRIGGERED_METADATA_KEY)
            .and_then(|value| match value.trim().to_ascii_lowercase().as_str() {
                "true" | "yes" | "1" | "triggered" | "fallback" => Some(true),
                "false" | "no" | "0" | "not_triggered" | "none" => Some(false),
                _ => None,
            })
    }
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
    /// Merge multiple reports into a single aggregated report.
    /// Results are concatenated in order. Total duration is the max of all suites.
    /// Timestamp is taken from the first report (or 0 if empty).
    pub fn merge(suite_name: impl Into<String>, reports: &[TestReport]) -> TestReport {
        let mut results = Vec::new();
        let mut total_duration = Duration::from_secs(0);
        let timestamp = reports.first().map(|r| r.timestamp).unwrap_or(0);

        for report in reports {
            results.extend(report.results.clone());
            if report.total_duration > total_duration {
                total_duration = report.total_duration;
            }
        }

        TestReport {
            suite_name: suite_name.into(),
            results,
            total_duration,
            timestamp,
        }
    }

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
