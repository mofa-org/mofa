//! MoFA Testing Framework
//!
//! Provides mock implementations, failure injection, and deterministic time
//! control for testing MoFA agents.

pub mod adversarial;
pub mod assertions;
pub mod backend;
pub mod behavior_diff;
pub mod bus;
pub mod clock;
pub mod report;
pub mod tools;

pub use backend::MockLLMBackend;
pub use behavior_diff::{
    BehaviorDiff, BehaviorDiffFormatter, BehaviorDiffSummary, CaseBehaviorDiff, CaseChangeKind,
    JsonBehaviorDiffFormatter, MarkdownBehaviorDiffFormatter, ValueChange,
};
pub use bus::MockAgentBus;
pub use clock::{Clock, MockClock, SystemClock};
pub use report::{
    AgentRunResult, BehaviorMetadata, FallbackStatus, JsonFormatter, ReportFormatter,
    TestCaseResult, TestReport, TestReportBuilder, TestStatus, TextFormatter,
};
pub use tools::MockTool;
