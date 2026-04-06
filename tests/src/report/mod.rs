//! Test report generation: collect results, compute stats, format output.

mod builder;
mod format;
mod types;

pub use builder::TestReportBuilder;
pub use format::{
    AllureExporter, AllureLabel, AllureParameter, AllureStatusDetails, AllureTestResult,
    JsonFormatter, ReportFormatter, TextFormatter,
};
pub use types::{TestCaseResult, TestReport, TestStatus};
