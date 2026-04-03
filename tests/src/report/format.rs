//! Formatters that render a [`TestReport`] to a string.

use crate::report::types::{TestReport, TestStatus};

/// Converts a [`TestReport`] into a displayable string.
pub trait ReportFormatter: Send + Sync {
    fn format(&self, report: &TestReport) -> String;
}

/// Renders a report as a JSON object.
pub struct JsonFormatter;

impl ReportFormatter for JsonFormatter {
    fn format(&self, report: &TestReport) -> String {
        let results: Vec<serde_json::Value> = report
            .results
            .iter()
            .map(|r| {
                let mut obj = serde_json::json!({
                    "name": r.name,
                    "status": r.status.to_string(),
                    "duration_ms": r.duration.as_millis() as u64,
                });
                if let Some(err) = &r.error {
                    obj["error"] = serde_json::Value::String(err.clone());
                }
                if !r.metadata.is_empty() {
                    let meta: serde_json::Map<String, serde_json::Value> = r
                        .metadata
                        .iter()
                        .map(|(k, v)| (k.clone(), serde_json::Value::String(v.clone())))
                        .collect();
                    obj["metadata"] = serde_json::Value::Object(meta);
                }
                obj
            })
            .collect();

        let total = report.total();
        let passed = report.passed();
        let failed = report.failed();
        let skipped = report.skipped();
        let pass_rate = report.pass_rate();

        let root = serde_json::json!({
            "suite": report.suite_name,
            "timestamp": report.timestamp,
            "total_duration_ms": report.total_duration.as_millis() as u64,
            "summary": {
                "total": total,
                "passed": passed,
                "failed": failed,
                "skipped": skipped,
                "pass_rate": pass_rate,
            },
            "results": results,
        });

        serde_json::to_string_pretty(&root).expect("report serialisation should not fail")
    }
}

/// Renders a report as a human-readable text block.
pub struct TextFormatter;

impl ReportFormatter for TextFormatter {
    fn format(&self, report: &TestReport) -> String {
        let mut buf = String::new();

        buf.push_str(&format!("=== {} ===\n", report.suite_name));

        for r in &report.results {
            let icon = match r.status {
                TestStatus::Passed => "+",
                TestStatus::Failed => "x",
                TestStatus::Skipped => "-",
            };
            buf.push_str(&format!(
                "[{}] {} .. {}ms\n",
                icon,
                r.name,
                r.duration.as_millis()
            ));
            if let Some(err) = &r.error {
                buf.push_str(&format!("     error: {}\n", err));
            }
        }

        buf.push_str(&format!(
            "\nTotal: {} | Passed: {} | Failed: {} | Skipped: {}\n",
            report.total(),
            report.passed(),
            report.failed(),
            report.skipped(),
        ));
        buf.push_str(&format!(
            "Pass rate: {:.1}% | Duration: {}ms\n",
            report.pass_rate() * 100.0,
            report.total_duration.as_millis(),
        ));

        buf
    }
}

fn escape_xml(input: &str) -> String {
    let mut escaped = String::with_capacity(input.len());
    for c in input.chars() {
        match c {
            '<' => escaped.push_str("&lt;"),
            '>' => escaped.push_str("&gt;"),
            '&' => escaped.push_str("&amp;"),
            '"' => escaped.push_str("&quot;"),
            '\'' => escaped.push_str("&apos;"),
            c if c == '\u{0009}' || c == '\u{000A}' || c == '\u{000D}' => escaped.push(c),
            c if c < '\u{0020}' => escaped.push_str("\u{FFFD}"), // Replace invalid control chars
            _ => escaped.push(c),
        }
    }
    escaped
}

/// Renders a report as a JUnit XML document.
pub struct JUnitFormatter;

impl ReportFormatter for JUnitFormatter {
    fn format(&self, report: &TestReport) -> String {
        let mut buf = String::new();
        let total = report.total();
        let failures = report.failed();
        let skipped = report.skipped();
        let time = report.total_duration.as_secs_f64();

        buf.push_str(&format!(
            "<testsuite name=\"mofa-testing\" tests=\"{}\" failures=\"{}\" errors=\"0\" skipped=\"{}\" time=\"{:.3}\">\n",
            total, failures, skipped, time
        ));

        for r in &report.results {
            let name = escape_xml(&r.name);
            let time = r.duration.as_secs_f64();

            buf.push_str(&format!(
                "  <testcase name=\"{}\" classname=\"mofa_testing\" time=\"{:.3}\"",
                name, time
            ));

            match r.status {
                TestStatus::Passed => {
                    buf.push_str("/>\n");
                }
                TestStatus::Skipped => {
                    buf.push_str(">\n    <skipped/>\n  </testcase>\n");
                }
                TestStatus::Failed => {
                    buf.push_str(">\n");
                    let (msg_attr, body) = match &r.error {
                        Some(err) => {
                            let summary = err.lines().next().unwrap_or("").chars().take(200).collect::<String>();
                            (escape_xml(&summary), escape_xml(err))
                        }
                        None => (String::new(), String::new()),
                    };
                    buf.push_str("    <failure message=\"");
                    buf.push_str(&msg_attr);
                    buf.push_str("\">");
                    buf.push_str(&body);
                    buf.push_str("</failure>\n");
                    buf.push_str("  </testcase>\n");
                }
            }
        }

        buf.push_str("</testsuite>\n");
        buf
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::report::types::{TestCaseResult, TestReport, TestStatus};
    use std::time::Duration;

    #[test]
    fn test_all_passing() {
        let report = TestReport {
            suite_name: "test_suite".to_string(),
            results: vec![
                TestCaseResult {
                    name: "test_one".to_string(),
                    status: TestStatus::Passed,
                    duration: Duration::from_millis(1500),
                    error: None,
                    metadata: vec![],
                },
                TestCaseResult {
                    name: "test_two".to_string(),
                    status: TestStatus::Passed,
                    duration: Duration::from_millis(250),
                    error: None,
                    metadata: vec![],
                },
            ],
            total_duration: Duration::from_millis(1750),
            timestamp: 1000,
        };

        let formatter = JUnitFormatter;
        let output = formatter.format(&report);

        let expected = "<testsuite name=\"mofa-testing\" tests=\"2\" failures=\"0\" errors=\"0\" skipped=\"0\" time=\"1.750\">\n  <testcase name=\"test_one\" classname=\"mofa_testing\" time=\"1.500\"/>\n  <testcase name=\"test_two\" classname=\"mofa_testing\" time=\"0.250\"/>\n</testsuite>\n";
        assert_eq!(output, expected);
    }

    #[test]
    fn test_mixed_status() {
        let report = TestReport {
            suite_name: "test_suite".to_string(),
            results: vec![
                TestCaseResult {
                    name: "test_pass".to_string(),
                    status: TestStatus::Passed,
                    duration: Duration::from_millis(100),
                    error: None,
                    metadata: vec![],
                },
                TestCaseResult {
                    name: "test_fail".to_string(),
                    status: TestStatus::Failed,
                    duration: Duration::from_millis(200),
                    error: Some("something went wrong".to_string()),
                    metadata: vec![],
                },
                TestCaseResult {
                    name: "test_skip".to_string(),
                    status: TestStatus::Skipped,
                    duration: Duration::from_millis(0),
                    error: None,
                    metadata: vec![],
                },
            ],
            total_duration: Duration::from_millis(300),
            timestamp: 1000,
        };

        let formatter = JUnitFormatter;
        let output = formatter.format(&report);

        let expected = "<testsuite name=\"mofa-testing\" tests=\"3\" failures=\"1\" errors=\"0\" skipped=\"1\" time=\"0.300\">\n  <testcase name=\"test_pass\" classname=\"mofa_testing\" time=\"0.100\"/>\n  <testcase name=\"test_fail\" classname=\"mofa_testing\" time=\"0.200\">\n    <failure message=\"something went wrong\">something went wrong</failure>\n  </testcase>\n  <testcase name=\"test_skip\" classname=\"mofa_testing\" time=\"0.000\">\n    <skipped/>\n  </testcase>\n</testsuite>\n";
        assert_eq!(output, expected);
    }

    #[test]
    fn test_case_with_error_payload() {
        let report = TestReport {
            suite_name: "test_suite".to_string(),
            results: vec![
                TestCaseResult {
                    name: "test_escaping_&_<'_\">".to_string(),
                    status: TestStatus::Failed,
                    duration: Duration::from_millis(123),
                    error: Some("Error: x < 5 & y > 10 with \"quotes\" and 'ticks'".to_string()),
                    metadata: vec![],
                },
            ],
            total_duration: Duration::from_millis(123),
            timestamp: 1000,
        };

        let formatter = JUnitFormatter;
        let output = formatter.format(&report);

        let expected = "<testsuite name=\"mofa-testing\" tests=\"1\" failures=\"1\" errors=\"0\" skipped=\"0\" time=\"0.123\">\n  <testcase name=\"test_escaping_&amp;_&lt;&apos;_&quot;&gt;\" classname=\"mofa_testing\" time=\"0.123\">\n    <failure message=\"Error: x &lt; 5 &amp; y &gt; 10 with &quot;quotes&quot; and &apos;ticks&apos;\">Error: x &lt; 5 &amp; y &gt; 10 with &quot;quotes&quot; and &apos;ticks&apos;</failure>\n  </testcase>\n</testsuite>\n";
        assert_eq!(output, expected);
    }

    #[test]
    fn test_case_with_metadata() {
        let report = TestReport {
            suite_name: "test_suite".to_string(),
            results: vec![
                TestCaseResult {
                    name: "test_metadata".to_string(),
                    status: TestStatus::Passed,
                    duration: Duration::from_millis(555),
                    error: None,
                    metadata: vec![("author".to_string(), "bob".to_string()), ("retries".to_string(), "2".to_string())],
                },
            ],
            total_duration: Duration::from_millis(555),
            timestamp: 1000,
        };

        let formatter = JUnitFormatter;
        let output = formatter.format(&report);

        let expected = "<testsuite name=\"mofa-testing\" tests=\"1\" failures=\"0\" errors=\"0\" skipped=\"0\" time=\"0.555\">\n  <testcase name=\"test_metadata\" classname=\"mofa_testing\" time=\"0.555\"/>\n</testsuite>\n";
        assert_eq!(output, expected);
    }

    #[test]
    fn test_chaotic_escaping() {
        let report = TestReport {
            suite_name: "test_suite".to_string(),
            results: vec![
                TestCaseResult {
                    name: "test_🚀_<![CDATA[...]]>".to_string(),
                    status: TestStatus::Failed,
                    duration: Duration::from_millis(0),
                    error: Some("HTML <b>hello</b>\0null bytes deeply nested \"'\"'\"'\"".to_string()),
                    metadata: vec![],
                },
            ],
            total_duration: Duration::from_millis(0),
            timestamp: 1000,
        };

        let formatter = JUnitFormatter;
        let output = formatter.format(&report);

        let expected = "<testsuite name=\"mofa-testing\" tests=\"1\" failures=\"1\" errors=\"0\" skipped=\"0\" time=\"0.000\">\n  <testcase name=\"test_🚀_&lt;![CDATA[...]]&gt;\" classname=\"mofa_testing\" time=\"0.000\">\n    <failure message=\"HTML &lt;b&gt;hello&lt;/b&gt;\u{FFFD}null bytes deeply nested &quot;&apos;&quot;&apos;&quot;&apos;&quot;\">HTML &lt;b&gt;hello&lt;/b&gt;\u{FFFD}null bytes deeply nested &quot;&apos;&quot;&apos;&quot;&apos;&quot;</failure>\n  </testcase>\n</testsuite>\n";
        assert_eq!(output, expected);
    }
}
