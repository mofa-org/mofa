//! Test report generation: collect results, compute stats, format output.

mod builder;
mod format;
mod types;

pub use builder::TestReportBuilder;
pub use format::{JsonFormatter, MarkdownFormatter, ReportFormatter, TextFormatter};
pub use types::{TestCaseResult, TestReport, TestStatus};
