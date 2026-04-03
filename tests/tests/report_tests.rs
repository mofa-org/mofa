use std::sync::Arc;
use std::time::Duration;

use mofa_testing::{
    JUnitFormatter, JsonFormatter, MockClock, ReportFormatter, TestCaseResult, TestReport,
    TestReportBuilder, TestStatus, TextFormatter,
};

// ---------------------------------------------------------------------------
// Helper
// ---------------------------------------------------------------------------

fn make_result(name: &str, status: TestStatus, ms: u64, error: Option<&str>) -> TestCaseResult {
    TestCaseResult {
        name: name.to_string(),
        status,
        duration: Duration::from_millis(ms),
        error: error.map(String::from),
        metadata: Vec::new(),
    }
}

fn mixed_report() -> TestReport {
    TestReport {
        suite_name: "mixed".into(),
        results: vec![
            make_result("a", TestStatus::Passed, 10, None),
            make_result("b", TestStatus::Failed, 50, Some("boom")),
            make_result("c", TestStatus::Passed, 30, None),
            make_result("d", TestStatus::Skipped, 0, None),
            make_result("e", TestStatus::Failed, 20, Some("oops")),
        ],
        total_duration: Duration::from_millis(110),
        timestamp: 1000,
    }
}

// ===========================================================================
// TestStatus
// ===========================================================================

#[test]
fn status_display() {
    assert_eq!(TestStatus::Passed.to_string(), "passed");
    assert_eq!(TestStatus::Failed.to_string(), "failed");
    assert_eq!(TestStatus::Skipped.to_string(), "skipped");
}

#[test]
fn status_equality() {
    assert_eq!(TestStatus::Passed, TestStatus::Passed);
    assert_ne!(TestStatus::Passed, TestStatus::Failed);
}

// ===========================================================================
// TestReport summary methods
// ===========================================================================

#[test]
fn report_counts() {
    let r = mixed_report();
    assert_eq!(r.total(), 5);
    assert_eq!(r.passed(), 2);
    assert_eq!(r.failed(), 2);
    assert_eq!(r.skipped(), 1);
}

#[test]
fn pass_rate_mixed() {
    let r = mixed_report();
    let rate = r.pass_rate();
    assert!((rate - 0.4).abs() < f64::EPSILON);
}

#[test]
fn pass_rate_all_pass() {
    let r = TestReport {
        suite_name: "ok".into(),
        results: vec![
            make_result("x", TestStatus::Passed, 1, None),
            make_result("y", TestStatus::Passed, 2, None),
        ],
        total_duration: Duration::ZERO,
        timestamp: 0,
    };
    assert!((r.pass_rate() - 1.0).abs() < f64::EPSILON);
}

#[test]
fn pass_rate_all_fail() {
    let r = TestReport {
        suite_name: "bad".into(),
        results: vec![make_result("z", TestStatus::Failed, 1, Some("err"))],
        total_duration: Duration::ZERO,
        timestamp: 0,
    };
    assert!((r.pass_rate()).abs() < f64::EPSILON);
}

#[test]
fn pass_rate_empty() {
    let r = TestReport {
        suite_name: "empty".into(),
        results: vec![],
        total_duration: Duration::ZERO,
        timestamp: 0,
    };
    assert!((r.pass_rate() - 1.0).abs() < f64::EPSILON);
}

#[test]
fn slowest_returns_descending() {
    let r = mixed_report();
    let top = r.slowest(3);
    assert_eq!(top.len(), 3);
    assert_eq!(top[0].name, "b"); // 50ms
    assert_eq!(top[1].name, "c"); // 30ms
    assert_eq!(top[2].name, "e"); // 20ms
}

#[test]
fn slowest_more_than_total() {
    let r = mixed_report();
    let top = r.slowest(100);
    assert_eq!(top.len(), 5);
}

#[test]
fn failures_returns_only_failed() {
    let r = mixed_report();
    let fails = r.failures();
    assert_eq!(fails.len(), 2);
    assert!(fails.iter().all(|f| f.status == TestStatus::Failed));
    assert_eq!(fails[0].name, "b");
    assert_eq!(fails[1].name, "e");
}

#[test]
fn failures_empty_when_all_pass() {
    let r = TestReport {
        suite_name: "ok".into(),
        results: vec![make_result("x", TestStatus::Passed, 1, None)],
        total_duration: Duration::ZERO,
        timestamp: 0,
    };
    assert!(r.failures().is_empty());
}

// ===========================================================================
// TestReportBuilder
// ===========================================================================

#[tokio::test]
async fn builder_record_passing() {
    let report = TestReportBuilder::new("pass-suite")
        .record("ok_test", || async { Ok(()) })
        .await
        .build();

    assert_eq!(report.suite_name, "pass-suite");
    assert_eq!(report.total(), 1);
    assert_eq!(report.results[0].status, TestStatus::Passed);
    assert!(report.results[0].error.is_none());
}

#[tokio::test]
async fn builder_record_failure() {
    let report = TestReportBuilder::new("fail-suite")
        .record("bad_test", || async { Err("kaboom".into()) })
        .await
        .build();

    assert_eq!(report.failed(), 1);
    assert_eq!(report.results[0].error.as_deref(), Some("kaboom"));
}

#[tokio::test]
async fn builder_add_result_skipped() {
    let report = TestReportBuilder::new("skip-suite")
        .add_result(make_result("skipped_one", TestStatus::Skipped, 0, None))
        .build();

    assert_eq!(report.skipped(), 1);
}

#[tokio::test]
async fn builder_empty_suite() {
    let report = TestReportBuilder::new("empty").build();
    assert_eq!(report.total(), 0);
    assert!((report.pass_rate() - 1.0).abs() < f64::EPSILON);
}

#[tokio::test]
async fn builder_with_mock_clock() {
    let clock = Arc::new(MockClock::starting_at(Duration::from_millis(42_000)));
    let report = TestReportBuilder::new("clocked").with_clock(clock).build();

    assert_eq!(report.timestamp, 42_000);
}

#[tokio::test]
async fn builder_mixed_record_and_add() {
    let report = TestReportBuilder::new("combo")
        .record("auto_pass", || async { Ok(()) })
        .await
        .add_result(make_result(
            "manual_fail",
            TestStatus::Failed,
            5,
            Some("err"),
        ))
        .record("auto_fail", || async { Err("nope".into()) })
        .await
        .add_result(make_result("manual_skip", TestStatus::Skipped, 0, None))
        .build();

    assert_eq!(report.total(), 4);
    assert_eq!(report.passed(), 1);
    assert_eq!(report.failed(), 2);
    assert_eq!(report.skipped(), 1);
}

// ===========================================================================
// JsonFormatter
// ===========================================================================

#[test]
fn json_formatter_valid_json() {
    let r = mixed_report();
    let output = JsonFormatter.format(&r);
    let parsed: serde_json::Value = serde_json::from_str(&output).expect("valid JSON");

    assert_eq!(parsed["suite"], "mixed");
    assert_eq!(parsed["timestamp"], 1000);
    assert_eq!(parsed["summary"]["total"], 5);
    assert_eq!(parsed["summary"]["passed"], 2);
    assert_eq!(parsed["summary"]["failed"], 2);
    assert_eq!(parsed["summary"]["skipped"], 1);

    let results = parsed["results"].as_array().expect("results array");
    assert_eq!(results.len(), 5);
    assert_eq!(results[0]["name"], "a");
    assert_eq!(results[0]["status"], "passed");
    assert_eq!(results[1]["error"], "boom");
}

#[test]
fn json_formatter_empty_report() {
    let r = TestReport {
        suite_name: "empty".into(),
        results: vec![],
        total_duration: Duration::ZERO,
        timestamp: 0,
    };
    let output = JsonFormatter.format(&r);
    let parsed: serde_json::Value = serde_json::from_str(&output).expect("valid JSON");
    assert_eq!(parsed["summary"]["total"], 0);
    assert_eq!(parsed["results"].as_array().unwrap().len(), 0);
}

#[test]
fn json_formatter_includes_metadata() {
    let mut tc = make_result("meta_test", TestStatus::Passed, 5, None);
    tc.metadata.push(("key".into(), "val".into()));
    let r = TestReport {
        suite_name: "m".into(),
        results: vec![tc],
        total_duration: Duration::from_millis(5),
        timestamp: 0,
    };
    let output = JsonFormatter.format(&r);
    let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();
    assert_eq!(parsed["results"][0]["metadata"]["key"], "val");
}

// ===========================================================================
// TextFormatter
// ===========================================================================

#[test]
fn text_formatter_contains_test_names() {
    let r = mixed_report();
    let output = TextFormatter.format(&r);
    assert!(output.contains("=== mixed ==="));
    assert!(output.contains("a"));
    assert!(output.contains("b"));
    assert!(output.contains("boom"));
}

#[test]
fn text_formatter_status_icons() {
    let r = mixed_report();
    let output = TextFormatter.format(&r);
    assert!(output.contains("[+]"));
    assert!(output.contains("[x]"));
    assert!(output.contains("[-]"));
}

#[test]
fn text_formatter_summary_line() {
    let r = mixed_report();
    let output = TextFormatter.format(&r);
    assert!(output.contains("Total: 5"));
    assert!(output.contains("Passed: 2"));
    assert!(output.contains("Failed: 2"));
    assert!(output.contains("Skipped: 1"));
    assert!(output.contains("Pass rate: 40.0%"));
}

#[test]
fn text_formatter_empty_report() {
    let r = TestReport {
        suite_name: "empty".into(),
        results: vec![],
        total_duration: Duration::ZERO,
        timestamp: 0,
    };
    let output = TextFormatter.format(&r);
    assert!(output.contains("Total: 0"));
    assert!(output.contains("Pass rate: 100.0%"));
}

// ===========================================================================
// JUnitFormatter
// ===========================================================================

#[test]
fn junit_formatter_all_passing_suite() {
    let report = TestReport {
        suite_name: "passing".into(),
        results: vec![
            make_result("alpha", TestStatus::Passed, 10, None),
            make_result("beta", TestStatus::Passed, 20, None),
        ],
        total_duration: Duration::from_millis(30),
        timestamp: 123,
    };

    let output = JUnitFormatter.format(&report);
    assert!(output.contains("<?xml version=\"1.0\" encoding=\"UTF-8\"?>"));
    assert!(output.contains("<testsuite name=\"passing\" tests=\"2\" failures=\"0\" skipped=\"0\" time=\"0.030\" timestamp=\"123\">"));
    assert!(output.contains("<testcase name=\"alpha\" time=\"0.010\"></testcase>"));
    assert!(output.contains("<testcase name=\"beta\" time=\"0.020\"></testcase>"));
}

#[test]
fn junit_formatter_mixed_status_suite() {
    let report = mixed_report();
    let output = JUnitFormatter.format(&report);

    assert!(output.contains("<testsuite name=\"mixed\" tests=\"5\" failures=\"2\" skipped=\"1\" time=\"0.110\" timestamp=\"1000\">"));
    assert!(output.contains("<failure message=\"boom\">boom</failure>"));
    assert!(output.contains("<failure message=\"oops\">oops</failure>"));
    assert!(output.contains("<skipped/>"));
}

#[test]
fn junit_formatter_escapes_error_payload() {
    let report = TestReport {
        suite_name: "xml".into(),
        results: vec![make_result(
            "needs<escape>",
            TestStatus::Failed,
            1,
            Some("bad <xml> & \"quotes\""),
        )],
        total_duration: Duration::from_millis(1),
        timestamp: 1,
    };

    let output = JUnitFormatter.format(&report);
    assert!(output.contains("name=\"needs&lt;escape&gt;\""));
    assert!(output.contains("message=\"bad &lt;xml&gt; &amp; &quot;quotes&quot;\""));
    assert!(output.contains(">bad &lt;xml&gt; &amp; &quot;quotes&quot;</failure>"));
}

#[test]
fn junit_formatter_includes_metadata_as_properties() {
    let mut tc = make_result("meta_case", TestStatus::Passed, 5, None);
    tc.metadata.push(("browser".into(), "webkit".into()));
    tc.metadata.push(("env".into(), "ci".into()));
    let report = TestReport {
        suite_name: "meta".into(),
        results: vec![tc],
        total_duration: Duration::from_millis(5),
        timestamp: 9,
    };

    let output = JUnitFormatter.format(&report);
    assert!(output.contains("<properties>"));
    assert!(output.contains("<property name=\"browser\" value=\"webkit\"/>"));
    assert!(output.contains("<property name=\"env\" value=\"ci\"/>"));
}

#[test]
fn junit_formatter_is_deterministic() {
    let report = mixed_report();
    let first = JUnitFormatter.format(&report);
    let second = JUnitFormatter.format(&report);
    assert_eq!(first, second);
}
