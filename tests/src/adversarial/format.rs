//! Formatters that render a [`SecurityReport`] to a string.

use crate::adversarial::policy::PolicyOutcome;
use crate::adversarial::report::SecurityReport;

/// Converts a [`SecurityReport`] into a displayable string.
pub trait SecurityReportFormatter: Send + Sync {
    fn format(&self, report: &SecurityReport) -> String;
}

/// Renders a [`SecurityReport`] as a JSON object.
pub struct SecurityJsonFormatter;

impl SecurityReportFormatter for SecurityJsonFormatter {
    fn format(&self, report: &SecurityReport) -> String {
        let root = serde_json::json!({
            "summary": {
                "total": report.total(),
                "passed": report.passed(),
                "failed": report.failed(),
                "pass_rate": report.pass_rate(),
            },
            "results": report.results,
        });

        serde_json::to_string_pretty(&root).expect("security report serialisation should not fail")
    }
}

/// Renders a [`SecurityReport`] in JUnit XML format.
///
/// Notes:
/// - This is intentionally minimal and CI-friendly.
/// - Each adversarial case maps to a single `<testcase>`.
pub struct SecurityJunitFormatter {
    /// Name used as the `<testsuite name="...">` attribute.
    pub suite_name: String,
    /// Classname used as the `<testcase classname="...">` attribute.
    pub class_name: String,
}

impl SecurityJunitFormatter {
    pub fn new(suite_name: impl Into<String>) -> Self {
        let suite_name = suite_name.into();
        let class_name = suite_name.clone();
        Self {
            suite_name,
            class_name,
        }
    }
}

impl SecurityReportFormatter for SecurityJunitFormatter {
    fn format(&self, report: &SecurityReport) -> String {
        let total = report.total();
        let failures = report.failed();

        let mut xml = String::new();
        xml.push_str(r#"<?xml version="1.0" encoding="UTF-8"?>"#);
        xml.push('\n');

        xml.push_str(&format!(
            r#"<testsuite name="{}" tests="{}" failures="{}">"#,
            xml_escape(&self.suite_name),
            total,
            failures
        ));
        xml.push('\n');

        for r in &report.results {
            xml.push_str(&format!(
                r#"  <testcase classname="{}" name="{}">"#,
                xml_escape(&self.class_name),
                xml_escape(&r.case_id)
            ));

            if let PolicyOutcome::Fail { reason } = &r.outcome {
                xml.push('\n');
                xml.push_str(&format!(
                    r#"    <failure message="{}">{}</failure>"#,
                    xml_escape(reason),
                    xml_escape(reason)
                ));
                xml.push('\n');
                xml.push_str("  </testcase>\n");
            } else {
                xml.push_str("</testcase>\n");
            }
        }

        xml.push_str("</testsuite>\n");
        xml
    }
}

fn xml_escape(s: &str) -> String {
    // Minimal escaping for XML attribute/text contexts.
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            _ => out.push(ch),
        }
    }
    out
}
