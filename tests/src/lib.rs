//! MoFA Testing Framework
//!
//! Provides mock implementations, a test-case DSL, assertion helpers,
//! a synchronous test runner, and JSON report generation.

pub mod backend;
pub mod bus;
pub mod case;
pub mod result;
pub mod suite;
pub mod tools;

pub use backend::MockLLMBackend;
pub use bus::MockAgentBus;
pub use case::{AgentTestCase, AgentTestCaseBuilder};
pub use result::{AssertionError, TestResult};
pub use suite::{SuiteReport, TestSuite};
pub use tools::MockTool;
