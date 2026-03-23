//! Behavioral diff support for comparing two test reports.

use crate::report::{TestCaseResult, TestReport, TestStatus};
use std::collections::{BTreeMap, BTreeSet};

const OUTPUT_KEYS: &[&str] = &["output", "final_response", "response"];
const TOOL_CALL_KEYS: &[&str] = &["tool_calls", "tool_call_sequence", "tools"];
const RETRY_KEYS: &[&str] = &["retry_count", "retries"];
const FALLBACK_KEYS: &[&str] = &["fallback_triggered", "fallback_status", "fallback"];

/// Comparison result for two test reports.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BehaviorDiff {
    pub baseline_suite: String,
    pub candidate_suite: String,
    pub summary: BehaviorDiffSummary,
    pub cases: Vec<CaseBehaviorDiff>,
}

/// Aggregate change counts for a diff.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct BehaviorDiffSummary {
    pub added_cases: usize,
    pub removed_cases: usize,
    pub status_changes: usize,
    pub output_changes: usize,
    pub tool_call_changes: usize,
    pub retry_changes: usize,
    pub fallback_changes: usize,
    pub slower_cases: usize,
    pub faster_cases: usize,
    pub unchanged_cases: usize,
    pub baseline_failed: usize,
    pub candidate_failed: usize,
    pub suite_duration_delta_ms: i128,
}

/// Per-case behavior change record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CaseBehaviorDiff {
    pub name: String,
    pub change: CaseChangeKind,
    pub status_change: Option<ValueChange<TestStatus>>,
    pub duration_delta_ms: i128,
    pub error_change: Option<ValueChange<Option<String>>>,
    pub output_change: Option<ValueChange<String>>,
    pub tool_calls_change: Option<ValueChange<String>>,
    pub retry_change: Option<ValueChange<i64>>,
    pub fallback_change: Option<ValueChange<bool>>,
}

/// High-level case presence/change classification.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CaseChangeKind {
    Added,
    Removed,
    Modified,
    Unchanged,
}

/// Generic before/after value change.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValueChange<T> {
    pub before: T,
    pub after: T,
}

impl BehaviorDiff {
    /// Compare a baseline and candidate report by test case name.
    pub fn between(baseline: &TestReport, candidate: &TestReport) -> Self {
        let baseline_cases: BTreeMap<&str, &TestCaseResult> = baseline
            .results
            .iter()
            .map(|case| (case.name.as_str(), case))
            .collect();
        let candidate_cases: BTreeMap<&str, &TestCaseResult> = candidate
            .results
            .iter()
            .map(|case| (case.name.as_str(), case))
            .collect();

        let names: BTreeSet<&str> = baseline_cases
            .keys()
            .copied()
            .chain(candidate_cases.keys().copied())
            .collect();

        let mut summary = BehaviorDiffSummary {
            baseline_failed: baseline.failed(),
            candidate_failed: candidate.failed(),
            suite_duration_delta_ms:
                candidate.total_duration.as_millis() as i128 - baseline.total_duration.as_millis() as i128,
            ..BehaviorDiffSummary::default()
        };
        let mut cases = Vec::new();

        for name in names {
            let diff = match (baseline_cases.get(name), candidate_cases.get(name)) {
                (None, Some(candidate_case)) => {
                    summary.added_cases += 1;
                    CaseBehaviorDiff {
                        name: name.to_string(),
                        change: CaseChangeKind::Added,
                        status_change: Some(ValueChange {
                            before: TestStatus::Skipped,
                            after: candidate_case.status.clone(),
                        }),
                        duration_delta_ms: candidate_case.duration.as_millis() as i128,
                        error_change: Some(ValueChange {
                            before: None,
                            after: candidate_case.error.clone(),
                        }),
                        output_change: metadata_change(None, Some(candidate_case), OUTPUT_KEYS),
                        tool_calls_change: metadata_change(None, Some(candidate_case), TOOL_CALL_KEYS),
                        retry_change: numeric_change(None, Some(candidate_case), RETRY_KEYS),
                        fallback_change: bool_change(None, Some(candidate_case), FALLBACK_KEYS),
                    }
                }
                (Some(baseline_case), None) => {
                    summary.removed_cases += 1;
                    CaseBehaviorDiff {
                        name: name.to_string(),
                        change: CaseChangeKind::Removed,
                        status_change: Some(ValueChange {
                            before: baseline_case.status.clone(),
                            after: TestStatus::Skipped,
                        }),
                        duration_delta_ms: -(baseline_case.duration.as_millis() as i128),
                        error_change: Some(ValueChange {
                            before: baseline_case.error.clone(),
                            after: None,
                        }),
                        output_change: metadata_change(Some(baseline_case), None, OUTPUT_KEYS),
                        tool_calls_change: metadata_change(Some(baseline_case), None, TOOL_CALL_KEYS),
                        retry_change: numeric_change(Some(baseline_case), None, RETRY_KEYS),
                        fallback_change: bool_change(Some(baseline_case), None, FALLBACK_KEYS),
                    }
                }
                (Some(baseline_case), Some(candidate_case)) => {
                    let status_change = if baseline_case.status != candidate_case.status {
                        summary.status_changes += 1;
                        Some(ValueChange {
                            before: baseline_case.status.clone(),
                            after: candidate_case.status.clone(),
                        })
                    } else {
                        None
                    };

                    let output_change =
                        metadata_change(Some(baseline_case), Some(candidate_case), OUTPUT_KEYS);
                    if output_change.is_some() {
                        summary.output_changes += 1;
                    }

                    let tool_calls_change =
                        metadata_change(Some(baseline_case), Some(candidate_case), TOOL_CALL_KEYS);
                    if tool_calls_change.is_some() {
                        summary.tool_call_changes += 1;
                    }

                    let retry_change =
                        numeric_change(Some(baseline_case), Some(candidate_case), RETRY_KEYS);
                    if retry_change.is_some() {
                        summary.retry_changes += 1;
                    }

                    let fallback_change =
                        bool_change(Some(baseline_case), Some(candidate_case), FALLBACK_KEYS);
                    if fallback_change.is_some() {
                        summary.fallback_changes += 1;
                    }

                    let error_change = if baseline_case.error != candidate_case.error {
                        Some(ValueChange {
                            before: baseline_case.error.clone(),
                            after: candidate_case.error.clone(),
                        })
                    } else {
                        None
                    };

                    let duration_delta_ms = candidate_case.duration.as_millis() as i128
                        - baseline_case.duration.as_millis() as i128;
                    if duration_delta_ms > 0 {
                        summary.slower_cases += 1;
                    } else if duration_delta_ms < 0 {
                        summary.faster_cases += 1;
                    }

                    let modified = status_change.is_some()
                        || output_change.is_some()
                        || tool_calls_change.is_some()
                        || retry_change.is_some()
                        || fallback_change.is_some()
                        || error_change.is_some()
                        || duration_delta_ms != 0;
                    if !modified {
                        summary.unchanged_cases += 1;
                    }

                    CaseBehaviorDiff {
                        name: name.to_string(),
                        change: if modified {
                            CaseChangeKind::Modified
                        } else {
                            CaseChangeKind::Unchanged
                        },
                        status_change,
                        duration_delta_ms,
                        error_change,
                        output_change,
                        tool_calls_change,
                        retry_change,
                        fallback_change,
                    }
                }
                (None, None) => unreachable!("name set only contains known cases"),
            };

            cases.push(diff);
        }

        Self {
            baseline_suite: baseline.suite_name.clone(),
            candidate_suite: candidate.suite_name.clone(),
            summary,
            cases,
        }
    }

    /// Render a readable markdown summary suitable for local review or CI comments.
    pub fn to_markdown(&self) -> String {
        let mut out = String::new();
        out.push_str(&format!(
            "## Behavioral Diff\n\nBaseline: `{}`\nCandidate: `{}`\n\n",
            self.baseline_suite, self.candidate_suite
        ));
        out.push_str(&format!(
            "- Cases added: {}\n- Cases removed: {}\n- Status changes: {}\n- Output changes: {}\n- Tool-call changes: {}\n- Retry changes: {}\n- Fallback changes: {}\n- Faster cases: {}\n- Slower cases: {}\n- Baseline failures: {}\n- Candidate failures: {}\n- Suite duration delta: {}ms\n",
            self.summary.added_cases,
            self.summary.removed_cases,
            self.summary.status_changes,
            self.summary.output_changes,
            self.summary.tool_call_changes,
            self.summary.retry_changes,
            self.summary.fallback_changes,
            self.summary.faster_cases,
            self.summary.slower_cases,
            self.summary.baseline_failed,
            self.summary.candidate_failed,
            self.summary.suite_duration_delta_ms,
        ));

        let interesting: Vec<&CaseBehaviorDiff> = self
            .cases
            .iter()
            .filter(|case| case.change != CaseChangeKind::Unchanged)
            .collect();
        if interesting.is_empty() {
            out.push_str("\nNo behavioral changes detected.\n");
            return out;
        }

        out.push_str("\n### Case Changes\n");
        for case in interesting {
            out.push_str(&format!("- `{}`: {}\n", case.name, case.describe()));
        }

        out
    }

    /// Render the diff as a JSON value suitable for artifacts or CI systems.
    pub fn to_json(&self) -> serde_json::Value {
        let cases: Vec<serde_json::Value> = self
            .cases
            .iter()
            .map(|case| {
                let mut value = serde_json::json!({
                    "name": case.name,
                    "change": case.change.as_str(),
                    "duration_delta_ms": case.duration_delta_ms,
                });
                if let Some(change) = &case.status_change {
                    value["status_change"] = serde_json::json!({
                        "before": change.before.to_string(),
                        "after": change.after.to_string(),
                    });
                }
                if let Some(change) = &case.error_change {
                    value["error_change"] = serde_json::json!({
                        "before": change.before,
                        "after": change.after,
                    });
                }
                if let Some(change) = &case.output_change {
                    value["output_change"] = string_change_json(change);
                }
                if let Some(change) = &case.tool_calls_change {
                    value["tool_calls_change"] = string_change_json(change);
                }
                if let Some(change) = &case.retry_change {
                    value["retry_change"] = serde_json::json!({
                        "before": change.before,
                        "after": change.after,
                    });
                }
                if let Some(change) = &case.fallback_change {
                    value["fallback_change"] = serde_json::json!({
                        "before": change.before,
                        "after": change.after,
                    });
                }
                value
            })
            .collect();

        serde_json::json!({
            "baseline_suite": self.baseline_suite,
            "candidate_suite": self.candidate_suite,
            "summary": {
                "added_cases": self.summary.added_cases,
                "removed_cases": self.summary.removed_cases,
                "status_changes": self.summary.status_changes,
                "output_changes": self.summary.output_changes,
                "tool_call_changes": self.summary.tool_call_changes,
                "retry_changes": self.summary.retry_changes,
                "fallback_changes": self.summary.fallback_changes,
                "slower_cases": self.summary.slower_cases,
                "faster_cases": self.summary.faster_cases,
                "unchanged_cases": self.summary.unchanged_cases,
                "baseline_failed": self.summary.baseline_failed,
                "candidate_failed": self.summary.candidate_failed,
                "suite_duration_delta_ms": self.summary.suite_duration_delta_ms,
            },
            "cases": cases,
        })
    }
}

impl CaseBehaviorDiff {
    fn describe(&self) -> String {
        let mut parts = Vec::new();
        match self.change {
            CaseChangeKind::Added => parts.push("added".to_string()),
            CaseChangeKind::Removed => parts.push("removed".to_string()),
            CaseChangeKind::Modified | CaseChangeKind::Unchanged => {}
        }
        if let Some(change) = &self.status_change {
            parts.push(format!("status {} -> {}", change.before, change.after));
        }
        if self.duration_delta_ms > 0 {
            parts.push(format!("slower by {}ms", self.duration_delta_ms));
        } else if self.duration_delta_ms < 0 {
            parts.push(format!("faster by {}ms", -self.duration_delta_ms));
        }
        if let Some(change) = &self.output_change {
            parts.push(format!("output changed ({} -> {})", change.before, change.after));
        }
        if let Some(change) = &self.tool_calls_change {
            parts.push(format!(
                "tool calls changed ({} -> {})",
                change.before, change.after
            ));
        }
        if let Some(change) = &self.retry_change {
            parts.push(format!("retry count {} -> {}", change.before, change.after));
        }
        if let Some(change) = &self.fallback_change {
            parts.push(format!("fallback {} -> {}", change.before, change.after));
        }
        if let Some(change) = &self.error_change {
            parts.push(format!(
                "error {:?} -> {:?}",
                change.before.as_deref(),
                change.after.as_deref()
            ));
        }
        if parts.is_empty() {
            "unchanged".to_string()
        } else {
            parts.join(", ")
        }
    }
}

impl CaseChangeKind {
    fn as_str(&self) -> &'static str {
        match self {
            Self::Added => "added",
            Self::Removed => "removed",
            Self::Modified => "modified",
            Self::Unchanged => "unchanged",
        }
    }
}

fn metadata_change(
    baseline: Option<&TestCaseResult>,
    candidate: Option<&TestCaseResult>,
    keys: &[&str],
) -> Option<ValueChange<String>> {
    let before = metadata_alias_value(baseline, keys);
    let after = metadata_alias_value(candidate, keys);
    if before != after {
        Some(ValueChange {
            before: before.unwrap_or_default().to_string(),
            after: after.unwrap_or_default().to_string(),
        })
    } else {
        None
    }
}

fn numeric_change(
    baseline: Option<&TestCaseResult>,
    candidate: Option<&TestCaseResult>,
    keys: &[&str],
) -> Option<ValueChange<i64>> {
    let before = metadata_alias_value(baseline, keys).and_then(|v| v.parse::<i64>().ok());
    let after = metadata_alias_value(candidate, keys).and_then(|v| v.parse::<i64>().ok());
    if before != after {
        Some(ValueChange {
            before: before.unwrap_or_default(),
            after: after.unwrap_or_default(),
        })
    } else {
        None
    }
}

fn bool_change(
    baseline: Option<&TestCaseResult>,
    candidate: Option<&TestCaseResult>,
    keys: &[&str],
) -> Option<ValueChange<bool>> {
    let before = metadata_alias_value(baseline, keys).and_then(parse_boolish);
    let after = metadata_alias_value(candidate, keys).and_then(parse_boolish);
    if before != after {
        Some(ValueChange {
            before: before.unwrap_or(false),
            after: after.unwrap_or(false),
        })
    } else {
        None
    }
}

fn metadata_alias_value<'a>(case: Option<&'a TestCaseResult>, keys: &[&str]) -> Option<&'a str> {
    let case = case?;
    keys.iter()
        .find_map(|key| case.metadata.iter().find(|(k, _)| k == key).map(|(_, v)| v.as_str()))
}

fn parse_boolish(value: &str) -> Option<bool> {
    match value.trim().to_ascii_lowercase().as_str() {
        "true" | "yes" | "1" | "triggered" | "fallback" => Some(true),
        "false" | "no" | "0" | "not_triggered" | "none" => Some(false),
        _ => None,
    }
}

fn string_change_json(change: &ValueChange<String>) -> serde_json::Value {
    serde_json::json!({
        "before": change.before,
        "after": change.after,
    })
}
