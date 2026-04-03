//! Formatters that render a [`TestReport`] to a string.

use crate::report::types::{TestReport, TestStatus};
use serde::Serialize;

/// Converts a [`TestReport`] into a displayable string.
pub trait ReportFormatter: Send + Sync {
    fn format(&self, report: &TestReport) -> String;
}

fn allure_status(status: &TestStatus) -> &'static str {
    match status {
        TestStatus::Passed => "passed",
        TestStatus::Failed => "failed",
        TestStatus::Skipped => "skipped",
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AllureLabel {
    pub name: String,
    pub value: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AllureParameter {
    pub name: String,
    pub value: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AllureStatusDetails {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AllureTestResult {
    pub uuid: String,
    pub history_id: String,
    pub name: String,
    pub full_name: String,
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status_details: Option<AllureStatusDetails>,
    pub labels: Vec<AllureLabel>,
    pub parameters: Vec<AllureParameter>,
    pub start: u64,
    pub stop: u64,
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

/// Exports a [`TestReport`] as Allure-compatible test result payloads.
pub struct AllureExporter;

impl AllureExporter {
    pub fn export(&self, report: &TestReport) -> Vec<AllureTestResult> {
        report
            .results
            .iter()
            .map(|result| {
                let full_name = format!("{}::{}", report.suite_name, result.name);
                let start = report.timestamp;
                let stop = report.timestamp + result.duration.as_millis() as u64;

                AllureTestResult {
                    uuid: full_name.clone(),
                    history_id: full_name.clone(),
                    name: result.name.clone(),
                    full_name,
                    status: allure_status(&result.status).to_string(),
                    status_details: result.error.as_ref().map(|message| AllureStatusDetails {
                        message: Some(message.clone()),
                    }),
                    labels: vec![
                        AllureLabel {
                            name: "suite".into(),
                            value: report.suite_name.clone(),
                        },
                        AllureLabel {
                            name: "framework".into(),
                            value: "mofa-testing".into(),
                        },
                    ],
                    parameters: result
                        .metadata
                        .iter()
                        .map(|(key, value)| AllureParameter {
                            name: key.clone(),
                            value: value.clone(),
                        })
                        .collect(),
                    start,
                    stop,
                }
            })
            .collect()
    }

    pub fn export_json(&self, report: &TestReport) -> Result<Vec<String>, serde_json::Error> {
        self.export(report)
            .into_iter()
            .map(|result| serde_json::to_string_pretty(&result))
            .collect()
    }
}
