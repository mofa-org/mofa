use mofa_testing::report::{TestReportBuilder, TestCaseResult, AllureFormatter, ReportFormatter};
use std::time::Duration;

#[test]
fn test_allure_format() {
    let mut builder = TestReportBuilder::new("AllureSuite");
    builder.add_result(TestCaseResult::passed("test_1", Duration::from_millis(15)));
    builder.add_result(TestCaseResult::failed("test_2", Duration::from_millis(20), "some err"));
    let report = builder.build();
    let fmt = AllureFormatter;
    let json = fmt.format(&report);
    assert!(json.contains("AllureSuite"));
    assert!(json.contains("passed"));
    assert!(json.contains("failed"));
    assert!(json.contains("some err"));
}
