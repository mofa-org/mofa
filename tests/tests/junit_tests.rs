use mofa_testing::report::{TestReportBuilder, TestCaseResult, JunitFormatter, ReportFormatter};
use std::time::Duration;

#[test]
fn test_junit_format() {
    let mut builder = TestReportBuilder::new("JunitSuite");
    builder.add_result(TestCaseResult::passed("test_1", Duration::from_millis(15)));
    builder.add_result(TestCaseResult::failed("test_2", Duration::from_millis(20), "some error"));
    let report = builder.build();
    let fmt = JunitFormatter;
    let xml = fmt.format(&report);
    assert!(xml.contains("testsuite name=\"JunitSuite\""));
    assert!(xml.contains("<failure message=\"some error\">some error</failure>"));
}