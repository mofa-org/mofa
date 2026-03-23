//! Fluent builder for collecting test results into a [`TestReport`].

use std::future::Future;
use std::sync::Arc;
use std::time::Instant;

use crate::clock::{Clock, SystemClock};
use crate::report::types::{TestCaseResult, TestReport, TestStatus};

/// Collects [`TestCaseResult`]s and produces a [`TestReport`].
pub struct TestReportBuilder {
    suite_name: String,
    clock: Arc<dyn Clock>,
    results: Vec<TestCaseResult>,
    suite_start: Instant,
}

impl TestReportBuilder {
    /// Create a new builder for the named suite.
    pub fn new(suite_name: impl Into<String>) -> Self {
        Self {
            suite_name: suite_name.into(),
            clock: Arc::new(SystemClock),
            results: Vec::new(),
            suite_start: Instant::now(),
        }
    }

    /// Inject a [`Clock`] implementation used for the report timestamp.
    pub fn with_clock(mut self, clock: Arc<dyn Clock>) -> Self {
        self.clock = clock;
        self
    }

    /// Run an async test closure and record its outcome.
    ///
    /// If the closure returns `Ok(())` the test is marked [`TestStatus::Passed`].
    /// If it returns `Err(msg)` the test is marked [`TestStatus::Failed`] with
    /// the error message captured.
    pub async fn record<F, Fut>(mut self, name: impl Into<String>, f: F) -> Self
    where
        F: FnOnce() -> Fut,
        Fut: Future<Output = Result<(), String>>,
    {
        let name = name.into();
        let start = Instant::now();
        let outcome = f().await;
        let duration = start.elapsed();

        let (status, error) = match outcome {
            Ok(()) => (TestStatus::Passed, None),
            Err(msg) => (TestStatus::Failed, Some(msg)),
        };

        self.results.push(TestCaseResult {
            name,
            status,
            duration,
            error,
            metadata: Vec::new(),
        });
        self
    }

    /// Run an async test closure, record its outcome, and attach metadata.
    pub async fn record_with_metadata<F, Fut, I, K, V>(
        mut self,
        name: impl Into<String>,
        metadata: I,
        f: F,
    ) -> Self
    where
        F: FnOnce() -> Fut,
        Fut: Future<Output = Result<(), String>>,
        I: IntoIterator<Item = (K, V)>,
        K: Into<String>,
        V: Into<String>,
    {
        let name = name.into();
        let start = Instant::now();
        let outcome = f().await;
        let duration = start.elapsed();

        let (status, error) = match outcome {
            Ok(()) => (TestStatus::Passed, None),
            Err(msg) => (TestStatus::Failed, Some(msg)),
        };

        self.results.push(
            TestCaseResult {
                name,
                status,
                duration,
                error,
                metadata: Vec::new(),
            }
            .with_metadata(metadata),
        );
        self
    }

    /// Manually add a pre-built result.
    pub fn add_result(mut self, result: TestCaseResult) -> Self {
        self.results.push(result);
        self
    }

    /// Consume the builder and produce a [`TestReport`].
    pub fn build(self) -> TestReport {
        TestReport {
            suite_name: self.suite_name,
            total_duration: self.suite_start.elapsed(),
            timestamp: self.clock.now_millis(),
            results: self.results,
        }
    }
}
