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

/// Renders a report as a Markdown summary.
pub struct MarkdownFormatter;

impl ReportFormatter for MarkdownFormatter {
    fn format(&self, report: &TestReport) -> String {
        let mut buf = String::new();
        buf.push_str(&format!("# Test Report: {}\n\n", report.suite_name));
        buf.push_str("## Summary\n\n");
        buf.push_str("| Total | Passed | Failed | Skipped | Pass Rate | Duration (ms) |\n");
        buf.push_str("| --- | --- | --- | --- | --- | --- |\n");
        buf.push_str(&format!(
            "| {} | {} | {} | {} | {:.1}% | {} |\n\n",
            report.total(),
            report.passed(),
            report.failed(),
            report.skipped(),
            report.pass_rate() * 100.0,
            report.total_duration.as_millis()
        ));

        buf.push_str("## Results\n\n");
        buf.push_str("| Status | Test Case | Duration (ms) | Error |\n");
        buf.push_str("| --- | --- | --- | --- |\n");

        for result in &report.results {
            let status = match result.status {
                TestStatus::Passed => "passed",
                TestStatus::Failed => "failed",
                TestStatus::Skipped => "skipped",
            };
            let error = result
                .error
                .as_deref()
                .map(escape_markdown_cell)
                .unwrap_or_else(|| "-".to_string());

            buf.push_str(&format!(
                "| {} | {} | {} | {} |\n",
                status,
                escape_markdown_cell(&result.name),
                result.duration.as_millis(),
                error
            ));
        }

        buf
    }
}

fn escape_markdown_cell(input: &str) -> String {
    input.replace('|', "\\|").replace('\n', "<br/>")
}
