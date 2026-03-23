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

/// Renders a report as a JUnit XML document for CI systems.
pub struct JunitFormatter;

impl ReportFormatter for JunitFormatter {
    fn format(&self, report: &TestReport) -> String {
        let mut xml = String::new();
        xml.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
        xml.push_str(&format!(
            "<testsuite name=\"{}\" tests=\"{}\" failures=\"{}\" skipped=\"{}\" time=\"{:.3}\" timestamp=\"{}\">\n",
            escape_xml(&report.suite_name),
            report.total(),
            report.failed(),
            report.skipped(),
            report.total_duration.as_secs_f64(),
            report.timestamp
        ));

        for result in &report.results {
            xml.push_str(&format!(
                "  <testcase name=\"{}\" classname=\"{}\" time=\"{:.3}\">",
                escape_xml(&result.name),
                escape_xml(&report.suite_name),
                result.duration.as_secs_f64()
            ));

            match result.status {
                TestStatus::Passed => {}
                TestStatus::Failed => {
                    let message = result.error.as_deref().unwrap_or("test failed");
                    xml.push_str(&format!(
                        "<failure message=\"{}\">{}</failure>",
                        escape_xml(message),
                        escape_xml(message)
                    ));
                }
                TestStatus::Skipped => {
                    let message = result.error.as_deref().unwrap_or("test skipped");
                    xml.push_str(&format!("<skipped message=\"{}\" />", escape_xml(message)));
                }
            }

            if !result.metadata.is_empty() {
                xml.push_str("<system-out>");
                for (key, value) in &result.metadata {
                    xml.push_str(&escape_xml(&format!("{key}={value}\n")));
                }
                xml.push_str("</system-out>");
            }

            xml.push_str("</testcase>\n");
        }

        xml.push_str("</testsuite>\n");
        xml
    }
}

fn escape_xml(input: &str) -> String {
    input
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}
