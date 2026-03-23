//! MoFA Testing Framework
//!
//! Provides mock implementations, failure injection, and deterministic time
//! control for testing MoFA agents.
//!
//! Contract fixtures live under `tests/fixtures/`. Prefer them when you need
//! deterministic, portable regression coverage across crates. Reach for ad hoc
//! integration tests only when a behavior cannot be described cleanly as a
//! reusable fixture.

pub mod assertions;
pub mod backend;
pub mod bus;
pub mod clock;
pub mod fixtures;
pub mod regression;
pub mod report;
pub mod tools;

pub use backend::MockLLMBackend;
pub use bus::MockAgentBus;
pub use clock::{Clock, MockClock, SystemClock};
pub use fixtures::{fixture_path, fixtures_root, load_fixture};
pub use regression::{assert_contains_all, assert_error_contains, assert_exact_match};
pub use report::{
    JsonFormatter, JunitFormatter, ReportFormatter, TestCaseResult, TestReport, TestReportBuilder,
    TestStatus, TextFormatter,
};
pub use tools::MockTool;
