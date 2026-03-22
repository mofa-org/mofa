//! Task Decomposer: LLM powered task analysis into a SubtaskDAG
//!
//! `deps` lists the `id`s of subtasks that must complete *before* this one
//! starts (Sequential dependency kind)

use serde::{Deserialize, Serialize};
use std::sync::{Arc, LazyLock};

use mofa_kernel::agent::types::error::{GlobalError, GlobalResult};

use crate::llm::provider::LLMProvider;
use crate::swarm::dag::{DependencyKind, RiskLevel, SubtaskDAG, SwarmSubtask};

// ── Keyword-based risk heuristics (compiled once, reused forever) ─────────────

static CRITICAL_RE: LazyLock<regex::Regex> = LazyLock::new(|| {
    regex::Regex::new(
        r"(?i)\b(delete|drop|payment|pay|deploy|publish|destroy|wipe|terminate|rm\b|kill)\b",
    )
    .expect("CRITICAL_RE is a valid regex")
});

static HIGH_RE: LazyLock<regex::Regex> = LazyLock::new(|| {
    regex::Regex::new(r"(?i)\b(write|create|post|send|update|modify|push|upload|insert)\b")
        .expect("HIGH_RE is a valid regex")
});

static LOW_RE: LazyLock<regex::Regex> = LazyLock::new(|| {
    regex::Regex::new(r"(?i)\b(read|search|fetch|get|list|find|summarize|summarise|view|show)\b")
        .expect("LOW_RE is a valid regex")
});

/// Classify a description string by matching against keyword sets.
/// Falls back to `Medium` when no keywords match.
fn classify_risk(description: &str) -> RiskLevel {
    if CRITICAL_RE.is_match(description) {
        RiskLevel::Critical
    } else if HIGH_RE.is_match(description) {
        RiskLevel::High
    } else if LOW_RE.is_match(description) {
        RiskLevel::Low
    } else {
        RiskLevel::Medium
    }
}

/// Estimate a duration from complexity: at least 10 s, at most 120 s.
fn estimate_duration(complexity: f64) -> u64 {
    ((complexity * 120.0) as u64).max(10)
}

/// Extract a JSON array block from LLM output.
fn extract_json_array(text: &str) -> &str {
    if let Some(start) = text.find("```json") {
        let content = &text[start + 7..];
        if let Some(end) = content.find("```") {
            return content[..end].trim();
        }
    }
    if let Some(start) = text.find("```") {
        let content = &text[start + 3..];
        if let Some(end) = content.find("```") {
            let block = content[..end].trim();
            if block.starts_with('[') {
                return block;
            }
        }
    }
    // 3. Bare [ ... ] only accept if the text starts with '[' (after trimming)
    let trimmed = text.trim();
    if let Some(start) = trimmed.find('[') {
        // Scan forward to find the matching closing bracket for the first array.
        let mut depth = 0usize;
        for (i, ch) in trimmed[start..].char_indices() {
            match ch {
                '[' => depth += 1,
                ']' => {
                    if depth == 0 {
                        continue;
                    }
                    depth -= 1;
                    if depth == 0 {
                        return &trimmed[start..start + i + 1];
                    }
                }
                _ => {}
            }
        }
        return &trimmed[start..];
    }
    text
}

/// Intermediate deserialization struct matching the LLM JSON contract
#[derive(Debug, Deserialize)]
struct SubtaskSpec {
    id: String,
    description: String,
    #[serde(default)]
    capabilities: Vec<String>,
    #[serde(default = "default_complexity")]
    complexity: f64,
    #[serde(default)]
    deps: Vec<String>,
}

fn default_complexity() -> f64 {
    0.5
}

/// Extended deserialization struct for the risk-aware LLM JSON contract.
///
/// This is intentionally separate from [`SubtaskSpec`] so the original
/// `analyze()` / `from_json()` methods remain completely unchanged.
#[derive(Debug, Deserialize)]
struct RiskAwareSubtaskSpec {
    id: String,
    description: String,
    #[serde(default)]
    capabilities: Vec<String>,
    #[serde(default = "default_complexity")]
    complexity: f64,
    #[serde(default)]
    deps: Vec<String>,
    /// Risk classification supplied by the LLM (or default `Low`).
    #[serde(default)]
    risk_level: RiskLevel,
    /// LLM-estimated duration in seconds (`None` when absent or 0).
    #[serde(default)]
    estimated_duration_secs: Option<u64>,
    /// LLM's one-sentence explanation of its risk classification.
    #[serde(default)]
    rationale: String,
}

// ── Public output types ───────────────────────────────────────────────────────

/// Count of subtasks grouped by risk level.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RiskSummary {
    pub low: usize,
    pub medium: usize,
    pub high: usize,
    pub critical: usize,
}

/// Rich output of [`TaskAnalyzer::analyze_with_risk`] and related methods.
///
/// In addition to the plain [`SubtaskDAG`] it includes HITL task IDs,
/// the computed critical path, and a per-risk-level summary.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RiskAwareAnalysis {
    /// The decomposed task graph with risk annotations on every node.
    pub dag: SubtaskDAG,
    /// IDs of subtasks that have `hitl_required = true`.
    pub hitl_required_tasks: Vec<String>,
    /// Ordered task IDs along the longest-duration (critical) path.
    pub critical_path: Vec<String>,
    /// Total estimated seconds along the critical path.
    pub critical_path_duration_secs: u64,
    /// Count of subtasks at each risk level.
    pub risk_summary: RiskSummary,
}

/// LLM powered task decomposer
pub struct TaskAnalyzer {
    provider: Arc<dyn LLMProvider>,
}

impl TaskAnalyzer {
    /// Create a new analyzer backed by the given LLM provider.
    pub fn new(provider: Arc<dyn LLMProvider>) -> Self {
        Self { provider }
    }

    // Decompose `task` by calling the LLM and parsing its JSON response
    pub async fn analyze(&self, task: &str) -> GlobalResult<SubtaskDAG> {
        use crate::llm::client::LLMClient;

        let client = LLMClient::new(self.provider.clone());

        let prompt = format!(
            "Decompose the following task into concrete subtasks.\n\
             Return ONLY a valid JSON array, no prose before or after.\n\
             Each element must have these fields:\n\
             - \"id\": short unique kebab-case string (e.g. \"step-1\")\n\
             - \"description\": one sentence describing the subtask\n\
             - \"capabilities\": list of capability tags needed (e.g. [\"llm\", \"web-search\"])\n\
             - \"complexity\": float 0.0–1.0 (0 = trivial, 1 = very hard)\n\
             - \"deps\": list of \"id\"s that must finish before this subtask starts\n\n\
             Task: {task}"
        );

        let response = client
            .chat()
            .system(
                "You are a task-decomposition engine that outputs structured JSON only. \
                 Never include explanatory text outside the JSON array.",
            )
            .user(prompt)
            .json_mode()
            .send()
            .await
            .map_err(|e| GlobalError::Other(format!("LLM call failed: {e}")))?;

        let raw = response
            .content()
            .ok_or_else(|| GlobalError::Other("LLM returned empty content".to_string()))?;

        Self::parse_json(task, raw)
    }

    //JSON string parsing into a SubtaskDAG
    pub fn from_json(task_name: &str, json: &str) -> GlobalResult<SubtaskDAG> {
        Self::parse_json(task_name, json)
    }

    fn parse_json(dag_name: &str, raw: &str) -> GlobalResult<SubtaskDAG> {
        // Extract the JSON block even if the LLM wrapped it in markdown fences.
        let json_str = extract_json_array(raw);

        let specs: Vec<SubtaskSpec> = serde_json::from_str(json_str).map_err(|e| {
            GlobalError::Other(format!(
                "Failed to parse LLM decomposition as JSON array: {e}\nRaw: {raw}"
            ))
        })?;

        if specs.is_empty() {
            return Err(GlobalError::Other(
                "LLM returned an empty subtask list".to_string(),
            ));
        }

        Self::build_dag(dag_name, specs)
    }

    fn build_dag(name: &str, specs: Vec<SubtaskSpec>) -> GlobalResult<SubtaskDAG> {
        let mut dag = SubtaskDAG::new(name);

        // First pass: validate uniqueness and add all subtask nodes
        let mut seen_ids = std::collections::HashSet::new();
        for spec in &specs {
            if spec.id.trim().is_empty() {
                return Err(GlobalError::Other(
                    "Subtask 'id' must not be empty".to_string(),
                ));
            }
            if !seen_ids.insert(spec.id.clone()) {
                return Err(GlobalError::Other(format!(
                    "Duplicate subtask id '{}' all ids must be unique",
                    spec.id
                )));
            }
            let subtask = SwarmSubtask::new(&spec.id, &spec.description)
                .with_capabilities(spec.capabilities.clone())
                .with_complexity(spec.complexity);
            dag.add_task(subtask);
        }

        // Second pass: wire up dependency edges using id → NodeIndex lookups
        for spec in &specs {
            for dep_id in &spec.deps {
                let from = dag.find_by_id(dep_id).ok_or_else(|| {
                    GlobalError::Other(format!(
                        "Dependency references unknown id '{dep_id}' in subtask '{}'",
                        spec.id
                    ))
                })?;
                let to = dag.find_by_id(&spec.id).ok_or_else(|| {
                    GlobalError::Other(format!("Subtask '{}' not found in DAG", spec.id))
                })?;
                dag.add_dependency_with_kind(from, to, DependencyKind::Sequential)
                    .map_err(|e| {
                        GlobalError::Other(format!(
                            "Dependency error ('{dep_id}' → '{}'): {e}",
                            spec.id
                        ))
                    })?;
            }
        }

        Ok(dag)
    }

    // ── Risk-aware public API ─────────────────────────────────────────────

    /// Decompose `task` using an enhanced prompt that asks the LLM to
    /// annotate each subtask with a `risk_level`, `estimated_duration_secs`,
    /// and a brief `rationale`.
    ///
    /// Returns a [`RiskAwareAnalysis`] containing the annotated DAG, HITL
    /// task IDs, critical path, and per-risk-level counts.
    pub async fn analyze_with_risk(&self, task: &str) -> GlobalResult<RiskAwareAnalysis> {
        use crate::llm::client::LLMClient;

        let client = LLMClient::new(self.provider.clone());

        let prompt = format!(
            "You are a task-decomposition and risk-assessment engine.\n\
             Output ONLY a valid JSON array — no prose before or after.\n\
             Each element must have exactly these fields:\n\
             - \"id\": short unique kebab-case string (e.g. \"step-1\")\n\
             - \"description\": one sentence describing the subtask\n\
             - \"capabilities\": list of capability tags needed (e.g. [\"llm\", \"web-search\"])\n\
             - \"complexity\": float 0.0–1.0 (0 = trivial, 1 = very hard)\n\
             - \"deps\": list of \"id\"s that must finish before this subtask starts\n\
             - \"risk_level\": one of \"low\", \"medium\", \"high\", \"critical\"\n\
             - \"estimated_duration_secs\": positive integer seconds (never 0)\n\
             - \"rationale\": one sentence explaining your risk classification\n\n\
             Risk classification guide:\n\
               low      — read-only, reversible, no external side-effects (web search, summarise)\n\
               medium   — writes to internal state, reversible with effort (draft doc, send email)\n\
               high     — writes to external systems or has significant impact (API call modifying data)\n\
               critical — irreversible, financial, security-sensitive, or production deployment\n\
                          (execute a payment, delete a database, deploy to production)\n\n\
             Rules:\n\
             - deps must reference ids already listed above this element\n\
             - The dependency graph must be acyclic\n\
             - estimated_duration_secs must be a positive integer (not 0)\n\n\
             Task: {task}"
        );

        let response = client
            .chat()
            .system(
                "You are a task-decomposition and risk-assessment engine that outputs \
                 structured JSON only. Never include explanatory text outside the JSON array. \
                 Classify each subtask by its real-world risk before providing the full response.",
            )
            .user(prompt)
            .json_mode()
            .send()
            .await
            .map_err(|e| GlobalError::Other(format!("LLM call failed: {e}")))?;

        let raw = response
            .content()
            .ok_or_else(|| GlobalError::Other("LLM returned empty content".to_string()))?;

        Self::parse_json_with_risk(task, raw)
    }

    /// Parse a pre-written risk-aware JSON string into a [`RiskAwareAnalysis`].
    ///
    /// The JSON must follow the extended format (8 fields per subtask).
    /// Missing `risk_level` and `estimated_duration_secs` fields default to
    /// `Low` and `None` respectively, so basic JSON from `analyze()` also parses.
    pub fn from_json_with_risk(task_name: &str, json: &str) -> GlobalResult<RiskAwareAnalysis> {
        Self::parse_json_with_risk(task_name, json)
    }

    /// Deterministic offline decomposition with keyword-based risk annotation.
    ///
    /// Splits the task on `" then "` or `" and "` (same as [`analyze_offline`]).
    /// Each step is classified by matching its description against keyword sets:
    /// - **Critical**: delete, payment, deploy, …
    /// - **High**: write, create, send, push, …
    /// - **Low**: read, search, fetch, summarize, …
    /// - **Medium**: everything else
    ///
    /// Duration is estimated from complexity: `max(10, complexity × 120)` seconds.
    pub fn analyze_offline_with_risk(task: &str) -> RiskAwareAnalysis {
        let mut dag = Self::analyze_offline(task);

        // Apply keyword-based risk classification and duration estimation to
        // every node (the offline DAG has no explicit risk from an LLM).
        let indices: Vec<_> = dag.all_tasks().iter().map(|(idx, _)| *idx).collect();
        for idx in indices {
            let (desc, complexity) = {
                let t = dag.get_task(idx).unwrap();
                (t.description.clone(), t.complexity)
            };
            let risk = classify_risk(&desc);
            let t = dag.get_task_mut(idx).unwrap();
            t.hitl_required = risk.requires_hitl();
            t.risk_level = risk;
            if t.estimated_duration_secs.is_none() {
                t.estimated_duration_secs = Some(estimate_duration(complexity));
            }
        }

        // Offline DAGs are always acyclic (built by hand), so critical_path()
        // is guaranteed to succeed.
        Self::finalize_analysis(dag)
            .expect("offline DAG is always cycle-free; critical_path() cannot fail")
    }

    // ── Private helpers ───────────────────────────────────────────────────

    fn parse_json_with_risk(dag_name: &str, raw: &str) -> GlobalResult<RiskAwareAnalysis> {
        let json_str = extract_json_array(raw);

        let specs: Vec<RiskAwareSubtaskSpec> = serde_json::from_str(json_str).map_err(|e| {
            GlobalError::Other(format!(
                "Failed to parse risk-aware decomposition as JSON array: {e}\nRaw: {raw}"
            ))
        })?;

        if specs.is_empty() {
            return Err(GlobalError::Other(
                "LLM returned an empty subtask list".to_string(),
            ));
        }

        Self::build_risk_dag(dag_name, specs)
    }

    fn build_risk_dag(
        name: &str,
        specs: Vec<RiskAwareSubtaskSpec>,
    ) -> GlobalResult<RiskAwareAnalysis> {
        let mut dag = SubtaskDAG::new(name);

        // First pass: validate uniqueness and add all subtask nodes.
        let mut seen_ids = std::collections::HashSet::new();
        for spec in &specs {
            if spec.id.trim().is_empty() {
                return Err(GlobalError::Other(
                    "Subtask 'id' must not be empty".to_string(),
                ));
            }
            if !seen_ids.insert(spec.id.clone()) {
                return Err(GlobalError::Other(format!(
                    "Duplicate subtask id '{}' — all ids must be unique",
                    spec.id
                )));
            }

            // Sanitise duration: treat 0 the same as None.
            let duration = spec.estimated_duration_secs.filter(|&d| d > 0);

            let subtask = SwarmSubtask::new(&spec.id, &spec.description)
                .with_capabilities(spec.capabilities.clone())
                .with_complexity(spec.complexity)
                .with_risk_level(spec.risk_level.clone());

            let subtask = if let Some(d) = duration {
                subtask.with_estimated_duration(d)
            } else {
                subtask
            };

            dag.add_task(subtask);
        }

        // Second pass: wire up dependency edges.
        for spec in &specs {
            for dep_id in &spec.deps {
                let from = dag.find_by_id(dep_id).ok_or_else(|| {
                    GlobalError::Other(format!(
                        "Dependency references unknown id '{dep_id}' in subtask '{}'",
                        spec.id
                    ))
                })?;
                let to = dag.find_by_id(&spec.id).ok_or_else(|| {
                    GlobalError::Other(format!("Subtask '{}' not found in DAG", spec.id))
                })?;
                dag.add_dependency_with_kind(from, to, DependencyKind::Sequential)
                    .map_err(|e| {
                        GlobalError::Other(format!(
                            "Dependency error ('{dep_id}' → '{}'): {e}",
                            spec.id
                        ))
                    })?;
            }
        }

        // Fill any missing estimated_duration_secs from complexity.
        let indices: Vec<_> = dag.all_tasks().iter().map(|(idx, _)| *idx).collect();
        for idx in indices {
            let (complexity, has_duration) = {
                let t = dag.get_task(idx).unwrap();
                (t.complexity, t.estimated_duration_secs.is_some())
            };
            if !has_duration {
                let t = dag.get_task_mut(idx).unwrap();
                t.estimated_duration_secs = Some(estimate_duration(complexity));
            }
        }

        Self::finalize_analysis(dag)
    }

    /// Compute the `RiskAwareAnalysis` wrapper around an already-built, fully
    /// annotated DAG (risk levels and durations already set on every node).
    fn finalize_analysis(dag: SubtaskDAG) -> GlobalResult<RiskAwareAnalysis> {
        let hitl_required_tasks = dag.hitl_required_tasks();
        let critical_path = dag.critical_path()?;
        let critical_path_duration_secs = dag.critical_path_duration_secs()?;
        let risk_summary = Self::compute_risk_summary(&dag);

        Ok(RiskAwareAnalysis {
            dag,
            hitl_required_tasks,
            critical_path,
            critical_path_duration_secs,
            risk_summary,
        })
    }

    fn compute_risk_summary(dag: &SubtaskDAG) -> RiskSummary {
        let mut summary = RiskSummary::default();
        for (_, task) in dag.all_tasks() {
            match task.risk_level {
                RiskLevel::Low => summary.low += 1,
                RiskLevel::Medium => summary.medium += 1,
                RiskLevel::High => summary.high += 1,
                RiskLevel::Critical => summary.critical += 1,
                _ => {}
            }
        }
        summary
    }

    /// Deterministic decomposition for unit tests
    pub fn analyze_offline(task: &str) -> SubtaskDAG {
        let task = task.trim();
        if task.is_empty() {
            let mut dag = SubtaskDAG::new("empty-task");
            dag.add_task(SwarmSubtask::new("step-1", "Do task"));
            return dag;
        }

        let lower = task.to_lowercase();
        let separator = if lower.contains(" then ") {
            Some(" then ")
        } else if lower.contains(" and ") {
            Some(" and ")
        } else {
            None
        };

        if let Some(sep) = separator {
            let char_pos = lower[..lower.find(sep).unwrap()].chars().count();
            let split_byte: usize = task.char_indices()
                .nth(char_pos)
                .map(|(b, _)| b)
                .unwrap_or(task.len());
            let first = task[..split_byte].trim();
            let sep_byte_in_original = sep.len(); // ASCII sep, bytes == chars
            let second = task[split_byte + sep_byte_in_original..].trim();

            let mut dag = SubtaskDAG::new(task);
            let idx1 = dag.add_task(SwarmSubtask::new("step-1", first).with_complexity(0.4));
            let idx2 = dag.add_task(SwarmSubtask::new("step-2", second).with_complexity(0.4));
            // step-2 depends on step-1 (sequential)
            let _ = dag.add_dependency(idx1, idx2);
            dag
        } else {
            let mut dag = SubtaskDAG::new(task);
            dag.add_task(SwarmSubtask::new("step-1", task).with_complexity(0.5));
            dag
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_from_json_single_task() {
        let json = r#"[{"id":"t1","description":"Do the thing","capabilities":["llm"],"complexity":0.3,"deps":[]}]"#;
        let dag = TaskAnalyzer::from_json("single", json).unwrap();
        assert_eq!(dag.task_count(), 1);
        assert!(dag.find_by_id("t1").is_some());
    }

    #[test]
    fn test_from_json_linear_chain() {
        let json = r#"[
            {"id":"a","description":"Step A","capabilities":[],"complexity":0.2,"deps":[]},
            {"id":"b","description":"Step B","capabilities":[],"complexity":0.3,"deps":["a"]},
            {"id":"c","description":"Step C","capabilities":[],"complexity":0.4,"deps":["b"]}
        ]"#;
        let dag = TaskAnalyzer::from_json("chain", json).unwrap();
        assert_eq!(dag.task_count(), 3);

        // Topological order: indices for a then b then c.
        let order = dag.topological_order().unwrap();
        assert_eq!(order.len(), 3);
        // a must be first, c must be last.
        let first = dag.get_task(order[0]).unwrap();
        let last = dag.get_task(*order.last().unwrap()).unwrap();
        assert_eq!(first.id, "a");
        assert_eq!(last.id, "c");
    }

    #[test]
    fn test_from_json_diamond_topology() {
        let json = r#"[
            {"id":"root","description":"Root","capabilities":[],"complexity":0.1,"deps":[]},
            {"id":"left","description":"Left","capabilities":[],"complexity":0.2,"deps":["root"]},
            {"id":"right","description":"Right","capabilities":[],"complexity":0.2,"deps":["root"]},
            {"id":"merge","description":"Merge","capabilities":[],"complexity":0.3,"deps":["left","right"]}
        ]"#;
        let dag = TaskAnalyzer::from_json("diamond", json).unwrap();
        assert_eq!(dag.task_count(), 4);

        let order = dag.topological_order().unwrap();
        let first = dag.get_task(order[0]).unwrap();
        let last = dag.get_task(*order.last().unwrap()).unwrap();
        assert_eq!(first.id, "root");
        assert_eq!(last.id, "merge");
    }

    #[test]
    fn test_from_json_cyclic_is_rejected() {
        let json = r#"[
            {"id":"a","description":"A","capabilities":[],"complexity":0.5,"deps":["b"]},
            {"id":"b","description":"B","capabilities":[],"complexity":0.5,"deps":["a"]}
        ]"#;
        let result = TaskAnalyzer::from_json("cycle", json);
        assert!(result.is_err(), "cyclic deps must return Err");
    }

    #[test]
    fn test_from_json_empty_array_is_rejected() {
        let result = TaskAnalyzer::from_json("empty", "[]");
        assert!(result.is_err(), "empty task list must return Err");
    }

    #[test]
    fn test_from_json_markdown_fenced_block() {
        let raw = "Here you go:\n```json\n[{\"id\":\"t1\",\"description\":\"Fetch data\",\"capabilities\":[\"http\"],\"complexity\":0.2,\"deps\":[]}]\n```";
        let dag = TaskAnalyzer::from_json("fenced", raw).unwrap();
        assert_eq!(dag.task_count(), 1);
    }

    #[test]
    fn test_from_json_capabilities_and_complexity() {
        let json = r#"[{"id":"t1","description":"Task","capabilities":["llm","search"],"complexity":0.8,"deps":[]}]"#;
        let dag = TaskAnalyzer::from_json("caps", json).unwrap();
        let idx = dag.find_by_id("t1").unwrap();
        let subtask = dag.get_task(idx).unwrap();
        assert_eq!(subtask.required_capabilities, vec!["llm", "search"]);
        assert!((subtask.complexity - 0.8).abs() < 1e-6);
    }

    #[test]
    fn test_from_json_unknown_dep_is_rejected() {
        let json = r#"[
            {"id":"b","description":"B","capabilities":[],"complexity":0.5,"deps":["nonexistent"]}
        ]"#;
        let result = TaskAnalyzer::from_json("bad-dep", json);
        assert!(result.is_err());
    }

    #[test]
    fn test_analyze_offline_empty_string() {
        let dag = TaskAnalyzer::analyze_offline("");
        assert_eq!(dag.task_count(), 1);
    }

    #[test]
    fn test_analyze_offline_single_task() {
        let dag = TaskAnalyzer::analyze_offline("Write a blog post about Rust");
        assert_eq!(dag.task_count(), 1);
        let idx = dag.find_by_id("step-1").unwrap();
        let t = dag.get_task(idx).unwrap();
        assert_eq!(t.description, "Write a blog post about Rust");
    }

    #[test]
    fn test_analyze_offline_then_splits_sequential() {
        let dag = TaskAnalyzer::analyze_offline("Fetch the data then summarise it");
        assert_eq!(dag.task_count(), 2);

        // Topological order
        let order = dag.topological_order().unwrap();
        let first = dag.get_task(order[0]).unwrap();
        let second = dag.get_task(order[1]).unwrap();
        assert_eq!(first.id, "step-1");
        assert_eq!(second.id, "step-2");
    }

    #[test]
    fn test_analyze_offline_and_splits_sequential() {
        let dag = TaskAnalyzer::analyze_offline("Research the topic and write a report");
        assert_eq!(dag.task_count(), 2);
        let order = dag.topological_order().unwrap();
        let first = dag.get_task(order[0]).unwrap();
        assert_eq!(first.id, "step-1");
    }

    #[test]
    fn test_analyze_offline_ready_tasks_initially_returns_first_only() {
        let dag = TaskAnalyzer::analyze_offline("Fetch the data then summarise it");
        // Only step 1 should be ready initially (step-2 depends on step-1)
        let ready = dag.ready_tasks();
        assert_eq!(ready.len(), 1);
        let ready_task = dag.get_task(ready[0]).unwrap();
        assert_eq!(ready_task.id, "step-1");
    }

    //edge case tests

    #[test]
    fn test_from_json_duplicate_id_is_rejected() {
        // Two subtasks with the same id must return Err.
        let json = r#"[
            {"id":"dup","description":"First","capabilities":[],"complexity":0.3,"deps":[]},
            {"id":"dup","description":"Second","capabilities":[],"complexity":0.3,"deps":[]}
        ]"#;
        let result = TaskAnalyzer::from_json("dup-test", json);
        assert!(result.is_err(), "duplicate ids must return Err");
        let msg = result.unwrap_err().to_string();
        assert!(msg.contains("Duplicate") || msg.contains("unique"), "error should mention duplicate: {msg}");
    }

    #[test]
    fn test_analyze_offline_unicode_does_not_panic() {
        // Tasks with non-ASCII characters must not panic
        let tasks = [
            "données then résultats",
            "データ and まとめ",
            "Ärger then Lösung",
        ];
        for task in tasks {
            // Should not panic outcome (1 or 2 subtasks) is acceptable either way
            let dag = TaskAnalyzer::analyze_offline(task);
            assert!(dag.task_count() >= 1, "expected at least 1 task for: {task}");
        }
    }

    #[test]
    fn test_extract_json_array_ignores_stray_trailing_brackets() {
        // A response that has stray [ ] after the array should still parse the array
        let raw = r#"[{"id":"t1","description":"D","capabilities":[],"complexity":0.5,"deps":[]}]
See also: reference [1] and [2]."#;
        let dag = TaskAnalyzer::from_json("stray", raw).unwrap();
        assert_eq!(dag.task_count(), 1);
    }

    // ── Risk-aware tests ──────────────────────────────────────────────────

    #[test]
    fn test_from_json_with_risk_single_task() {
        let json = r#"[{
            "id":"t1","description":"Search for data","capabilities":["llm"],
            "complexity":0.3,"deps":[],
            "risk_level":"low","estimated_duration_secs":15,"rationale":"read-only"
        }]"#;
        let analysis = TaskAnalyzer::from_json_with_risk("single-risk", json).unwrap();
        assert_eq!(analysis.dag.task_count(), 1);
        let idx = analysis.dag.find_by_id("t1").unwrap();
        let t = analysis.dag.get_task(idx).unwrap();
        assert_eq!(t.risk_level, RiskLevel::Low);
        assert!(!t.hitl_required);
        assert_eq!(t.estimated_duration_secs, Some(15));
    }

    #[test]
    fn test_from_json_with_risk_hitl_required_tasks() {
        let json = r#"[
            {"id":"a","description":"Search data","capabilities":[],"complexity":0.2,"deps":[],
             "risk_level":"low","estimated_duration_secs":10,"rationale":"read"},
            {"id":"b","description":"Send payment","capabilities":[],"complexity":0.8,"deps":["a"],
             "risk_level":"critical","estimated_duration_secs":5,"rationale":"financial"},
            {"id":"c","description":"Update record","capabilities":[],"complexity":0.5,"deps":["b"],
             "risk_level":"high","estimated_duration_secs":20,"rationale":"writes external"}
        ]"#;
        let analysis = TaskAnalyzer::from_json_with_risk("hitl-test", json).unwrap();
        let mut hitl = analysis.hitl_required_tasks.clone();
        hitl.sort();
        assert_eq!(hitl, vec!["b", "c"]);
    }

    #[test]
    fn test_from_json_with_risk_low_medium_not_hitl() {
        let json = r#"[
            {"id":"a","description":"Read file","capabilities":[],"complexity":0.1,"deps":[],
             "risk_level":"low","estimated_duration_secs":5,"rationale":"read-only"},
            {"id":"b","description":"Draft document","capabilities":[],"complexity":0.4,"deps":[],
             "risk_level":"medium","estimated_duration_secs":30,"rationale":"internal write"}
        ]"#;
        let analysis = TaskAnalyzer::from_json_with_risk("no-hitl", json).unwrap();
        assert!(analysis.hitl_required_tasks.is_empty());
    }

    #[test]
    fn test_from_json_with_risk_critical_path_computed() {
        // a(10) → b(20) → c(30)  — only one path, total 60
        let json = r#"[
            {"id":"a","description":"A","capabilities":[],"complexity":0.1,"deps":[],
             "risk_level":"low","estimated_duration_secs":10,"rationale":"r"},
            {"id":"b","description":"B","capabilities":[],"complexity":0.3,"deps":["a"],
             "risk_level":"low","estimated_duration_secs":20,"rationale":"r"},
            {"id":"c","description":"C","capabilities":[],"complexity":0.5,"deps":["b"],
             "risk_level":"medium","estimated_duration_secs":30,"rationale":"r"}
        ]"#;
        let analysis = TaskAnalyzer::from_json_with_risk("cp-test", json).unwrap();
        assert_eq!(analysis.critical_path, vec!["a", "b", "c"]);
        assert_eq!(analysis.critical_path_duration_secs, 60);
    }

    #[test]
    fn test_from_json_with_risk_risk_summary_counts() {
        let json = r#"[
            {"id":"l1","description":"L1","capabilities":[],"complexity":0.1,"deps":[],
             "risk_level":"low","estimated_duration_secs":5,"rationale":"r"},
            {"id":"l2","description":"L2","capabilities":[],"complexity":0.1,"deps":[],
             "risk_level":"low","estimated_duration_secs":5,"rationale":"r"},
            {"id":"h1","description":"H1","capabilities":[],"complexity":0.7,"deps":[],
             "risk_level":"high","estimated_duration_secs":30,"rationale":"r"},
            {"id":"c1","description":"C1","capabilities":[],"complexity":0.9,"deps":[],
             "risk_level":"critical","estimated_duration_secs":60,"rationale":"r"}
        ]"#;
        let analysis = TaskAnalyzer::from_json_with_risk("summary", json).unwrap();
        assert_eq!(analysis.risk_summary.low, 2);
        assert_eq!(analysis.risk_summary.medium, 0);
        assert_eq!(analysis.risk_summary.high, 1);
        assert_eq!(analysis.risk_summary.critical, 1);
    }

    #[test]
    fn test_from_json_with_risk_missing_fields_use_defaults() {
        // Minimal JSON without risk_level / estimated_duration_secs / rationale
        let json = r#"[{"id":"t1","description":"Do something","capabilities":[],"complexity":0.5,"deps":[]}]"#;
        let analysis = TaskAnalyzer::from_json_with_risk("defaults", json).unwrap();
        assert_eq!(analysis.dag.task_count(), 1);
        // risk defaults to Low; estimated_duration filled from complexity
        let idx = analysis.dag.find_by_id("t1").unwrap();
        let t = analysis.dag.get_task(idx).unwrap();
        assert_eq!(t.risk_level, RiskLevel::Low);
        assert!(t.estimated_duration_secs.is_some());
    }

    #[test]
    fn test_analyze_offline_with_risk_payment_keyword_is_critical() {
        let analysis = TaskAnalyzer::analyze_offline_with_risk("pay the invoice");
        assert_eq!(analysis.dag.task_count(), 1);
        let idx = analysis.dag.find_by_id("step-1").unwrap();
        let t = analysis.dag.get_task(idx).unwrap();
        assert_eq!(t.risk_level, RiskLevel::Critical, "description contains 'pay'");
        assert!(t.hitl_required);
    }

    #[test]
    fn test_analyze_offline_with_risk_delete_keyword_is_critical() {
        let analysis = TaskAnalyzer::analyze_offline_with_risk("delete old records");
        let idx = analysis.dag.find_by_id("step-1").unwrap();
        let t = analysis.dag.get_task(idx).unwrap();
        assert_eq!(t.risk_level, RiskLevel::Critical, "description contains 'delete'");
    }

    #[test]
    fn test_analyze_offline_with_risk_search_keyword_is_low() {
        let analysis = TaskAnalyzer::analyze_offline_with_risk("search for recent papers");
        let idx = analysis.dag.find_by_id("step-1").unwrap();
        let t = analysis.dag.get_task(idx).unwrap();
        assert_eq!(t.risk_level, RiskLevel::Low, "description contains 'search'");
        assert!(!t.hitl_required);
    }

    #[test]
    fn test_analyze_offline_with_risk_two_steps_one_high() {
        // "search" → Low; "send" → High
        let analysis =
            TaskAnalyzer::analyze_offline_with_risk("search for contacts then send email");
        assert_eq!(analysis.dag.task_count(), 2);
        let mut hitl = analysis.hitl_required_tasks.clone();
        hitl.sort();
        // Only "step-2" (send email) should require HITL
        assert_eq!(hitl, vec!["step-2"]);
    }

    /// Live LLM integration test works with any OpenAI compatible provider
    #[tokio::test]
    #[ignore = "requires LLM_API_KEY env var — run locally only"]
    async fn test_analyze_with_live_llm() {
        use crate::llm::openai::{OpenAIConfig, OpenAIProvider};
        use std::sync::Arc;

        let api_key  = std::env::var("LLM_API_KEY").expect("Set LLM_API_KEY to run this test");
        let base_url = std::env::var("LLM_BASE_URL").expect("Set LLM_BASE_URL to run this test");
        let model    = std::env::var("LLM_MODEL").expect("Set LLM_MODEL to run this test");

        let provider = OpenAIProvider::with_config(
            OpenAIConfig::new(api_key)
                .with_base_url(base_url)
                .with_model(model)
                .with_max_tokens(512),
        );

        let analyzer = TaskAnalyzer::new(Arc::new(provider));
        let dag = analyzer
            .analyze("Research quantum computing then write a short summary")
            .await
            .expect("LLM decomposition should succeed");

        println!("\n=== TaskAnalyzer live LLM integration ===\n");
        println!("DAG name : {}", dag.name);
        println!("Task count: {}", dag.task_count());
        for (idx, task) in dag.all_tasks() {
            println!(
                "  [{:?}] {} — caps: {:?}  complexity: {:.1}",
                idx, task.description, task.required_capabilities, task.complexity
            );
        }
        let order = dag.topological_order().unwrap();
        println!("\nTopological order:");
        for idx in &order {
            let t = dag.get_task(*idx).unwrap();
            println!("  {} (deps: {:?})", t.id, dag.dependencies_of(*idx));
        }

        assert!(dag.task_count() >= 1, "Expected at least one subtask from the LLM");
        assert!(
            dag.topological_order().is_ok(),
            "Expected a cycle-free DAG from the LLM"
        );
    }
}
