use mofa_testing::behavior_diff::BehaviorDiff;
use mofa_testing::report::{TestCaseResult, TestReport, TestStatus};
use std::time::Duration;

fn make_result(
    name: &str,
    status: TestStatus,
    duration_ms: u64,
    error: Option<&str>,
    metadata: &[(&str, &str)],
) -> TestCaseResult {
    TestCaseResult {
        name: name.to_string(),
        status,
        duration: Duration::from_millis(duration_ms),
        error: error.map(ToString::to_string),
        metadata: metadata
            .iter()
            .map(|(key, value)| (key.to_string(), value.to_string()))
            .collect(),
    }
}

fn main() {
    let baseline = TestReport {
        suite_name: "baseline".to_string(),
        total_duration: Duration::from_millis(90),
        timestamp: 0,
        results: vec![
            make_result(
                "answer-question",
                TestStatus::Passed,
                20,
                None,
                &[
                    ("output", "Paris"),
                    ("tool_calls", "knowledge_base"),
                    ("retry_count", "0"),
                    ("fallback_triggered", "false"),
                ],
            ),
            make_result("stable-case", TestStatus::Passed, 10, None, &[]),
        ],
    };

    let candidate = TestReport {
        suite_name: "candidate".to_string(),
        total_duration: Duration::from_millis(125),
        timestamp: 0,
        results: vec![
            make_result(
                "answer-question",
                TestStatus::Failed,
                55,
                Some("wrong fact"),
                &[
                    ("output", "Lyon"),
                    ("tool_calls", "knowledge_base,search"),
                    ("retry_count", "2"),
                    ("fallback_triggered", "true"),
                ],
            ),
            make_result("stable-case", TestStatus::Passed, 10, None, &[]),
            make_result(
                "new-case",
                TestStatus::Passed,
                12,
                None,
                &[("response", "new coverage")],
            ),
        ],
    };

    let diff = BehaviorDiff::between(&baseline, &candidate);

    println!("== Markdown Summary ==");
    println!("{}", diff.to_markdown());

    println!("== JSON Summary ==");
    println!(
        "{}",
        serde_json::to_string_pretty(&diff.to_json()).expect("behavior diff JSON should serialize")
    );
}
