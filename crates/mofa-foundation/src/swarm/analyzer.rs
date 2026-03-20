//! Task Decomposer: LLM powered task analysis into a SubtaskDAG
//!
//! `deps` lists the `id`s of subtasks that must complete *before* this one
//! starts (Sequential dependency kind)

use serde::Deserialize;
use std::sync::Arc;

use mofa_kernel::agent::types::error::{GlobalError, GlobalResult};

use crate::llm::provider::LLMProvider;
use crate::swarm::dag::{DependencyKind, SubtaskDAG, SwarmSubtask};

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
