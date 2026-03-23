//! MoFA Testing Framework
//!
//! Provides mock implementations, failure injection, deterministic time control,
//! and concurrency testing utilities for testing MoFA agents.

pub mod adversarial;
pub mod assertions;
pub mod backend;
pub mod bus;
pub mod clock;
pub mod concurrency;
pub mod report;
pub mod tools;

pub use backend::MockLLMBackend;
pub use bus::MockAgentBus;
pub use clock::{Clock, MockClock, SystemClock};
pub use concurrency::ConcurrencyTestBuilder;
pub use report::{
    JsonFormatter, ReportFormatter, TestCaseResult, TestReport, TestReportBuilder, TestStatus,
    TextFormatter,
};
pub use tools::MockTool;
