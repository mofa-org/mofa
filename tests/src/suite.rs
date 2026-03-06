//! Test suite runner and report generation for the MoFA testing framework.
//!
//! [`TestSuite`] collects [`AgentTestCase`]s and runs them synchronously
//! against a [`MoFAAgent`].  Results are collected into a [`SuiteReport`]
//! which supports JSON serialization and human-readable summary output.

use crate::case::AgentTestCase;
use crate::result::TestResult;
use mofa_kernel::agent::context::AgentContext;
use mofa_kernel::agent::core::MoFAAgent;
use serde::Serialize;
use std::time::Instant;

// ============================================================================
// TestSuite
// ============================================================================

/// A named collection of [`AgentTestCase`]s.
pub struct TestSuite {
    name: String,
    cases: Vec<AgentTestCase>,
}

impl TestSuite {
    /// Create an empty suite with the given name.
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            cases: Vec::new(),
        }
    }

    /// Append a test case to the suite (fluent).
    pub fn add(mut self, case: AgentTestCase) -> Self {
        self.cases.push(case);
        self
    }

    /// Run all cases against `agent` synchronously and return a [`SuiteReport`].
    ///
    /// A fresh [`AgentContext`] is created for each case.  Mock injection is
    /// the caller's responsibility — pass an already-configured agent.
    ///
    /// # Panics
    ///
    /// Panics if the internal Tokio runtime cannot be created.  This should
    /// never happen in practice.
    pub fn run<A: MoFAAgent>(&self, agent: &mut A) -> SuiteReport {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_time()
            .build()
            .expect("mofa-testing: failed to create tokio runtime for TestSuite::run");

        let suite_start = Instant::now();
        let mut results: Vec<TestResult> = Vec::with_capacity(self.cases.len());

        for case in &self.cases {
            let input = case.input.clone();
            let case_name = case.name.clone();
            let timeout_ms = case.timeout_ms;

            let ctx = AgentContext::new(format!("test-{}", case_name));

            let case_start = Instant::now();

            let outcome = rt.block_on(async {
                if let Some(ms) = timeout_ms {
                    let duration = std::time::Duration::from_millis(ms);
                    match tokio::time::timeout(duration, agent.execute(input, &ctx)).await {
                        Ok(result) => result.map_err(|e| e.to_string()),
                        Err(_) => Err(format!("timed out after {}ms", ms)),
                    }
                } else {
                    agent.execute(input, &ctx).await.map_err(|e| e.to_string())
                }
            });

            let duration_ms = case_start.elapsed().as_millis() as u64;
            let final_state = Some(agent.state());

            let (output, error, passed) = match outcome {
                Ok(out) => (Some(out), None, true),
                Err(msg) => (None, Some(msg), false),
            };

            results.push(TestResult {
                case_name,
                output,
                final_state,
                error,
                duration_ms,
                passed,
            });
        }

        let total_duration_ms = suite_start.elapsed().as_millis() as u64;
        let passed = results.iter().filter(|r| r.passed).count();
        let failed = results.len() - passed;

        SuiteReport {
            suite_name: self.name.clone(),
            total: results.len(),
            passed,
            failed,
            results,
            total_duration_ms,
            timestamp: chrono::Utc::now(),
        }
    }
}

// ============================================================================
// SuiteReport
// ============================================================================

/// Aggregated results of a [`TestSuite`] run.
pub struct SuiteReport {
    pub suite_name: String,
    pub total: usize,
    pub passed: usize,
    pub failed: usize,
    pub results: Vec<TestResult>,
    pub total_duration_ms: u64,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

impl SuiteReport {
    /// Return `true` if every test case passed.
    pub fn all_passed(&self) -> bool {
        self.failed == 0
    }

    /// Serialize the report to a pretty-printed JSON string.
    pub fn to_json(&self) -> String {
        let serializable = SerializableSuiteReport::from(self);
        serde_json::to_string_pretty(&serializable).expect("SuiteReport serialization failed")
    }

    /// Write the JSON report to `path`.
    pub fn write_json(&self, path: &str) -> std::io::Result<()> {
        std::fs::write(path, self.to_json())
    }

    /// Print a human-readable summary to stdout.
    pub fn print_summary(&self) {
        println!("Suite: {}", self.suite_name);
        println!(
            "  {} total, {} passed, {} failed  ({}ms)",
            self.total, self.passed, self.failed, self.total_duration_ms
        );
        for r in &self.results {
            let status = if r.passed { "PASS" } else { "FAIL" };
            if let Some(err) = &r.error {
                println!("  [{}] {} — {}", status, r.case_name, err);
            } else {
                println!("  [{}] {}  ({}ms)", status, r.case_name, r.duration_ms);
            }
        }
    }
}

// ============================================================================
// Serialization helpers
// ============================================================================

/// Serializable projection of [`SuiteReport`] for JSON output.
#[derive(Serialize)]
struct SerializableSuiteReport<'a> {
    suite_name: &'a str,
    total: usize,
    passed: usize,
    failed: usize,
    total_duration_ms: u64,
    timestamp: String,
    results: Vec<SerializableTestResult<'a>>,
}

impl<'a> From<&'a SuiteReport> for SerializableSuiteReport<'a> {
    fn from(r: &'a SuiteReport) -> Self {
        Self {
            suite_name: &r.suite_name,
            total: r.total,
            passed: r.passed,
            failed: r.failed,
            total_duration_ms: r.total_duration_ms,
            timestamp: r.timestamp.to_rfc3339(),
            results: r.results.iter().map(SerializableTestResult::from).collect(),
        }
    }
}

/// Serializable projection of [`TestResult`] for JSON output.
#[derive(Serialize)]
struct SerializableTestResult<'a> {
    case_name: &'a str,
    passed: bool,
    duration_ms: u64,
    error: Option<&'a str>,
}

impl<'a> From<&'a TestResult> for SerializableTestResult<'a> {
    fn from(r: &'a TestResult) -> Self {
        Self {
            case_name: &r.case_name,
            passed: r.passed,
            duration_ms: r.duration_ms,
            error: r.error.as_deref(),
        }
    }
}
