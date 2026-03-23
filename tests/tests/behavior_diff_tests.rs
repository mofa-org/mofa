use mofa_testing::behavior_diff::{
    BehaviorDiff, BehaviorDiffFormatter, CaseChangeKind, JsonBehaviorDiffFormatter,
    MarkdownBehaviorDiffFormatter,
};
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
        name: name.into(),
        status,
        duration: Duration::from_millis(duration_ms),
        error: error.map(String::from),
        metadata: metadata
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect(),
    }
}

fn make_report(name: &str, duration_ms: u64, results: Vec<TestCaseResult>) -> TestReport {
    TestReport {
        suite_name: name.into(),
        results,
        total_duration: Duration::from_millis(duration_ms),
        timestamp: 0,
    }
}

#[test]
fn behavior_diff_detects_status_and_latency_changes() {
    let baseline = make_report(
        "baseline",
        100,
        vec![make_result("agent-case", TestStatus::Passed, 10, None, &[])],
    );
    let candidate = make_report(
        "candidate",
        150,
        vec![make_result(
            "agent-case",
            TestStatus::Failed,
            45,
            Some("tool timeout"),
            &[],
        )],
    );

    let diff = BehaviorDiff::between(&baseline, &candidate);

    assert_eq!(diff.summary.status_changes, 1);
    assert_eq!(diff.summary.slower_cases, 1);
    assert_eq!(diff.summary.suite_duration_delta_ms, 50);

    let case = &diff.cases[0];
    assert_eq!(case.change, CaseChangeKind::Modified);
    assert_eq!(
        case.status_change.as_ref().map(|c| (&c.before, &c.after)),
        Some((&TestStatus::Passed, &TestStatus::Failed))
    );
    assert_eq!(case.duration_delta_ms, 35);
    assert_eq!(
        case.error_change.as_ref().map(|c| (c.before.as_deref(), c.after.as_deref())),
        Some((None, Some("tool timeout")))
    );
}

#[test]
fn behavior_diff_detects_metadata_behavior_changes() {
    let baseline = make_report(
        "baseline",
        100,
        vec![make_result(
            "tool-case",
            TestStatus::Passed,
            30,
            None,
            &[
                ("output", "answer-a"),
                ("tool_calls", "search"),
                ("retry_count", "0"),
                ("fallback_triggered", "false"),
            ],
        )],
    );
    let candidate = make_report(
        "candidate",
        100,
        vec![make_result(
            "tool-case",
            TestStatus::Passed,
            20,
            None,
            &[
                ("output", "answer-b"),
                ("tool_calls", "search,lookup"),
                ("retry_count", "2"),
                ("fallback_triggered", "true"),
            ],
        )],
    );

    let diff = BehaviorDiff::between(&baseline, &candidate);
    let case = &diff.cases[0];

    assert_eq!(diff.summary.output_changes, 1);
    assert_eq!(diff.summary.tool_call_changes, 1);
    assert_eq!(diff.summary.retry_changes, 1);
    assert_eq!(diff.summary.fallback_changes, 1);
    assert_eq!(diff.summary.faster_cases, 1);

    assert_eq!(
        case.output_change.as_ref().map(|c| (c.before.as_str(), c.after.as_str())),
        Some(("answer-a", "answer-b"))
    );
    assert_eq!(
        case.tool_calls_change
            .as_ref()
            .map(|c| (c.before.as_str(), c.after.as_str())),
        Some(("search", "search,lookup"))
    );
    assert_eq!(
        case.retry_change.as_ref().map(|c| (c.before, c.after)),
        Some((0, 2))
    );
    assert_eq!(
        case.fallback_change.as_ref().map(|c| (c.before, c.after)),
        Some((false, true))
    );
}

#[test]
fn behavior_diff_detects_added_removed_and_markdown_summary() {
    let baseline = make_report(
        "baseline",
        100,
        vec![
            make_result("kept", TestStatus::Passed, 10, None, &[]),
            make_result("removed", TestStatus::Passed, 5, None, &[]),
        ],
    );
    let candidate = make_report(
        "candidate",
        120,
        vec![
            make_result("kept", TestStatus::Passed, 10, None, &[]),
            make_result("added", TestStatus::Passed, 20, None, &[("response", "hi")]),
        ],
    );

    let diff = BehaviorDiff::between(&baseline, &candidate);

    assert_eq!(diff.summary.added_cases, 1);
    assert_eq!(diff.summary.removed_cases, 1);
    assert_eq!(diff.summary.unchanged_cases, 1);
    assert!(diff
        .cases
        .iter()
        .any(|case| case.name == "added" && case.change == CaseChangeKind::Added));
    assert!(diff
        .cases
        .iter()
        .any(|case| case.name == "removed" && case.change == CaseChangeKind::Removed));

    let markdown = diff.to_markdown();
    assert!(markdown.contains("## Behavioral Diff"));
    assert!(markdown.contains("Cases added: 1"));
    assert!(markdown.contains("`added`: added"));
    assert!(markdown.contains("`removed`: removed"));
}

#[test]
fn behavior_diff_json_contains_summary_and_case_details() {
    let baseline = make_report(
        "baseline",
        90,
        vec![make_result(
            "case-a",
            TestStatus::Passed,
            10,
            None,
            &[("output", "old"), ("retry_count", "0")],
        )],
    );
    let candidate = make_report(
        "candidate",
        100,
        vec![make_result(
            "case-a",
            TestStatus::Failed,
            15,
            Some("bad output"),
            &[("output", "new"), ("retry_count", "1")],
        )],
    );

    let diff = BehaviorDiff::between(&baseline, &candidate);
    let json = diff.to_json();

    assert_eq!(json["baseline_suite"], "baseline");
    assert_eq!(json["candidate_suite"], "candidate");
    assert_eq!(json["summary"]["status_changes"], 1);
    assert_eq!(json["summary"]["retry_changes"], 1);
    assert_eq!(json["cases"][0]["name"], "case-a");
    assert_eq!(json["cases"][0]["change"], "modified");
    assert_eq!(json["cases"][0]["status_change"]["before"], "passed");
    assert_eq!(json["cases"][0]["status_change"]["after"], "failed");
    assert_eq!(json["cases"][0]["output_change"]["before"], "old");
    assert_eq!(json["cases"][0]["output_change"]["after"], "new");
}

#[test]
fn behavior_diff_has_change_helpers() {
    let baseline = make_report(
        "baseline",
        20,
        vec![make_result("same", TestStatus::Passed, 10, None, &[])],
    );
    let candidate = make_report(
        "candidate",
        20,
        vec![make_result("same", TestStatus::Passed, 10, None, &[])],
    );

    let unchanged = BehaviorDiff::between(&baseline, &candidate);
    assert!(!unchanged.has_changes());
    assert!(unchanged.changed_cases().is_empty());

    let changed_candidate = make_report(
        "candidate",
        30,
        vec![make_result("same", TestStatus::Failed, 20, Some("oops"), &[])],
    );
    let changed = BehaviorDiff::between(&baseline, &changed_candidate);
    assert!(changed.has_changes());
    assert_eq!(changed.changed_cases().len(), 1);
    assert_eq!(changed.changed_cases()[0].name, "same");
}

#[test]
fn behavior_diff_formatters_render_markdown_and_json() {
    let baseline = make_report(
        "baseline",
        10,
        vec![make_result("case", TestStatus::Passed, 5, None, &[("output", "old")])],
    );
    let candidate = make_report(
        "candidate",
        20,
        vec![make_result("case", TestStatus::Failed, 9, Some("err"), &[("output", "new")])],
    );

    let diff = BehaviorDiff::between(&baseline, &candidate);

    let markdown = MarkdownBehaviorDiffFormatter.format(&diff);
    assert!(markdown.contains("## Behavioral Diff"));
    assert!(markdown.contains("status passed -> failed"));

    let json = JsonBehaviorDiffFormatter.format(&diff);
    let parsed: serde_json::Value = serde_json::from_str(&json).expect("valid diff json");
    assert_eq!(parsed["summary"]["status_changes"], 1);
    assert_eq!(parsed["cases"][0]["status_change"]["after"], "failed");
}
