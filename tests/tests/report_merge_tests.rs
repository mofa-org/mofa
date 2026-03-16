use mofa_testing::report::{TestCaseResult, TestReport, TestStatus};
use std::time::Duration;

fn make_report(
    name: &str,
    results: Vec<TestCaseResult>,
    duration: u64,
    timestamp: u64,
) -> TestReport {
    TestReport {
        suite_name: name.into(),
        results,
        total_duration: Duration::from_secs(duration),
        timestamp,
    }
}

fn make_result(name: &str, status: TestStatus, duration: u64) -> TestCaseResult {
    TestCaseResult {
        name: name.into(),
        status,
        duration: Duration::from_secs(duration),
        error: None,
        metadata: vec![],
    }
}

#[tokio::test]
async fn merge_two_reports_concatenates_results() {
    let r1 = make_report(
        "a",
        vec![make_result("t1", TestStatus::Passed, 1)],
        10,
        1000,
    );
    let r2 = make_report(
        "b",
        vec![make_result("t2", TestStatus::Failed, 2)],
        20,
        2000,
    );

    let merged = TestReport::merge("combined", &[r1, r2]);

    assert_eq!(merged.suite_name, "combined");
    assert_eq!(merged.results.len(), 2);
    assert_eq!(merged.results[0].name, "t1");
    assert_eq!(merged.results[1].name, "t2");
}

#[tokio::test]
async fn merge_preserves_counts() {
    let r1 = make_report(
        "a",
        vec![
            make_result("t1", TestStatus::Passed, 1),
            make_result("t2", TestStatus::Failed, 1),
        ],
        10,
        1000,
    );
    let r2 = make_report(
        "b",
        vec![
            make_result("t3", TestStatus::Passed, 1),
            make_result("t4", TestStatus::Skipped, 1),
        ],
        10,
        2000,
    );

    let merged = TestReport::merge("combined", &[r1, r2]);

    assert_eq!(merged.total(), 4);
    assert_eq!(merged.passed(), 2);
    assert_eq!(merged.failed(), 1);
    assert_eq!(merged.skipped(), 1);
}

#[tokio::test]
async fn merge_uses_max_duration_and_first_timestamp() {
    let r1 = make_report("a", vec![], 10, 1000);
    let r2 = make_report("b", vec![], 50, 2000);
    let r3 = make_report("c", vec![], 30, 3000);

    let merged = TestReport::merge("combined", &[r1, r2, r3]);

    assert_eq!(merged.total_duration, Duration::from_secs(50));
    assert_eq!(merged.timestamp, 1000);
}

#[tokio::test]
async fn merge_zero_reports_returns_empty() {
    let merged = TestReport::merge("combined", &[]);

    assert_eq!(merged.suite_name, "combined");
    assert_eq!(merged.results.len(), 0);
    assert_eq!(merged.total_duration, Duration::from_secs(0));
    assert_eq!(merged.timestamp, 0);
}

#[tokio::test]
async fn merged_report_pass_rate_is_correct() {
    let r1 = make_report(
        "a",
        vec![make_result("t1", TestStatus::Passed, 1)],
        10,
        1000,
    );
    let r2 = make_report(
        "b",
        vec![
            make_result("t2", TestStatus::Passed, 1),
            make_result("t3", TestStatus::Failed, 1),
            make_result("t4", TestStatus::Failed, 1),
        ],
        10,
        2000,
    );

    let merged = TestReport::merge("c", &[r1, r2]);
    assert_eq!(merged.pass_rate(), 0.5); // 2 passed out of 4 total
}

#[tokio::test]
async fn slowest_on_merged_report() {
    let r1 = make_report(
        "a",
        vec![
            make_result("t1", TestStatus::Passed, 5),
            make_result("t2", TestStatus::Passed, 15),
        ],
        10,
        1000,
    );
    let r2 = make_report(
        "b",
        vec![
            make_result("t3", TestStatus::Passed, 20),
            make_result("t4", TestStatus::Passed, 2),
        ],
        10,
        2000,
    );

    let merged = TestReport::merge("c", &[r1, r2]);
    let slowest = merged.slowest(2);

    assert_eq!(slowest.len(), 2);
    assert_eq!(slowest[0].name, "t3"); // 20
    assert_eq!(slowest[1].name, "t2"); // 15
}
