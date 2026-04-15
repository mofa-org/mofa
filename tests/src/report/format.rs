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

/// Renders a report as JUnit XML format.
pub struct JunitFormatter;

impl ReportFormatter for JunitFormatter {
    fn format(&self, report: &TestReport) -> String {
        let mut buf = String::new();
        buf.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
        buf.push_str(&format!(
            "<testsuite name=\"{}\" tests=\"{}\" failures=\"{}\" skipped=\"{}\" time=\"{:.3}\" timestamp=\"{}\">\n",
            report.suite_name,
            report.total(), report.failed(), report.skipped(),
            report.total_duration.as_secs_f64(),
            report.timestamp
        ));

        for r in &report.results {
            buf.push_str(&format!(
                "  <testcase classname=\"{}\" name=\"{}\" time=\"{:.3}\">\n",
                report.suite_name, r.name, r.duration.as_secs_f64()
            ));

            match r.status {
                TestStatus::Passed => {}
                TestStatus::Failed => {
                    let err_msg = r.error.as_deref().unwrap_or("Test failed");
                    let safe_msg = err_msg.replace("&", "&amp;").replace("<", "&lt;").replace(">", "&gt;").replace("\"", "&quot;").replace("'", "&apos;");
                    buf.push_str(&format!("    <failure message=\"{}\">{}</failure>\n", safe_msg, safe_msg));
                }
                TestStatus::Skipped => {
                    buf.push_str("    <skipped/>\n");
                }
            }

            if !r.metadata.is_empty() {
                buf.push_str("    <system-out><![CDATA[\n");
                for (k, v) in &r.metadata {
                    buf.push_str(&format!("{}: {}\n", k, v));
                }
                buf.push_str("    ]]></system-out>\n");
            }
            buf.push_str("  </testcase>\n");
        }

        buf.push_str("</testsuite>\n");
        buf
    }
}

/// Renders a report as Allure JSON format.
pub struct AllureFormatter;

impl ReportFormatter for AllureFormatter {
    fn format(&self, report: &TestReport) -> String {
        let results: Vec<serde_json::Value> = report
            .results
            .iter()
            .map(|r| {
                let status = match r.status {
                    TestStatus::Passed => "passed",
                    TestStatus::Failed => "failed",
                    TestStatus::Skipped => "skipped",
                };
                
                let mut obj = serde_json::json!({
                    "name": r.name,
                    "status": status,
                    "statusDetails": {"message": r.error.clone().unwrap_or_default()},
                    "start": report.timestamp, // Rough approx
                    "stop": report.timestamp + r.duration.as_millis() as u64,
                    "labels": [
                        {"name": "suite", "value": report.suite_name}
                    ]
                });

                if !r.metadata.is_empty() {
                    let params: Vec<serde_json::Value> = r.metadata.iter().map(|(k, v)| {
                        serde_json::json!({"name": k, "value": v})
                    }).collect();
                    obj["parameters"] = params;
                }
                
                obj
            })
            .collect();
            
        // Usually allure writes multiple files, but for the formatter trait we can return a JSON array or list of objects
        serde_json::to_string_pretty(&results).expect("allure serialization failed")
    }
}

