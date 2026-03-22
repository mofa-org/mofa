//! `mofa swarm run` command implementation
//!
//! Reads a YAML swarm config plus DAG, executes it through the scheduler,
//! and prints streaming colored progress logs.

use crate::CliError;
use colored::Colorize;
use mofa_foundation::swarm::{
    AgentSpec, SchedulerSummary, SubtaskDAG, SubtaskExecutorFn, SwarmConfig, SwarmScheduler,
    SwarmSubtask, TaskExecutionContext,
};
use mofa_sdk::llm::{LLMAgentBuilder, OpenAIConfig, OpenAIProvider};
use serde::Deserialize;
use std::path::Path;
use std::sync::Arc;
use std::time::Instant;

#[derive(Debug, Deserialize)]
struct SwarmRunFile {
    #[serde(flatten)]
    config: SwarmConfig,
    executor: SwarmExecutorKind,
    dag: Option<SubtaskDAG>,
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
enum SwarmExecutorKind {
    Llm,
}

/// Execute `mofa swarm run <file>`
pub async fn run_swarm(file: &Path, json_output: bool) -> Result<(), CliError> {
    // 1. Read & Parse YAML 
    let yaml = std::fs::read_to_string(file).map_err(|e| {
        CliError::Other(format!(
            "Failed to read swarm config '{}': {}",
            file.display(),
            e
        ))
    })?;

    let run_file: SwarmRunFile = serde_yaml::from_str(&yaml).map_err(|e| {
        CliError::Other(format!(
            "Failed to parse swarm YAML '{}': {}",
            file.display(),
            e
        ))
    })?;

    let config = run_file.config;
    // 2. Load DAG 
    let mut dag = run_file.dag.ok_or_else(|| {
        CliError::Other("Swarm YAML must include a `dag` section".to_string())
    })?;
    if dag.name.is_empty() {
        dag.name = config.name.clone();
    }

    print_swarm_header(&config, dag.task_count(), run_file.executor);

    // 3. Build executor 
    let executor = build_llm_executor(&config, &config.agents)?;

    // 4. Run with the appropriate scheduler 
    let scheduler = config.pattern.clone().into_scheduler();
    let wall_start = Instant::now();

    let summary = scheduler
        .execute(&mut dag, executor)
        .await
        .map_err(|e| CliError::Other(format!("Swarm execution failed: {e}")))?;

    let wall_ms = wall_start.elapsed().as_millis();

    // 5. Print result 
    print_summary(&summary, wall_ms);

    if json_output {
        let json = serde_json::to_string_pretty(&summary)
            .map_err(|e| CliError::Other(format!("JSON serialisation failed: {e}")))?;
        println!("\n{}", json);
    }

    if summary.failed > 0 {
        return Err(CliError::Other(format!(
            "{} task(s) failed during swarm execution",
            summary.failed
        )));
    }

    Ok(())
}

//  Helpers 

/// Print a colourful banner before execution begins.
fn print_swarm_header(config: &SwarmConfig, task_count: usize, executor: SwarmExecutorKind) {
    println!();
    println!("{} {}", "◈ MoFA Swarm".bold().cyan(), config.name.bold());
    if !config.description.is_empty() {
        println!("  {}", config.description.dimmed());
    }
    println!(
        "  {} {}",
        "Pattern:".dimmed(),
        format!("{}", config.pattern).yellow()
    );
    println!(
        "  {} {} agent(s)",
        "Agents:".dimmed(),
        config.agents.len().to_string().yellow()
    );
    println!(
        "  {} {} task(s)",
        "DAG:".dimmed(),
        task_count.to_string().yellow()
    );
    println!(
        "  {} {}",
        "Executor:".dimmed(),
        match executor {
            SwarmExecutorKind::Llm => "LLM".green(),
        }
    );
    println!("  {} {}", "Task:".dimmed(), config.task.italic());
    println!();
}

fn build_llm_executor(
    config: &SwarmConfig,
    agents: &[AgentSpec],
) -> Result<SubtaskExecutorFn, CliError> {
    let api_key = std::env::var("OPENAI_API_KEY").map_err(|_| {
        CliError::Other("LLM executor requires OPENAI_API_KEY in the environment".to_string())
    })?;
    let base_url = std::env::var("OPENAI_BASE_URL").ok();
    let default_model = std::env::var("OPENAI_MODEL").ok();

    let agents = agents.to_vec();
    let swarm_task = config.task.clone();
    let swarm_name = config.name.clone();

    Ok(Arc::new(move |_idx, task, context| {
        // Clone captured config because the executor is invoked per task.
        let api_key = api_key.clone();
        let base_url = base_url.clone();
        let default_model = default_model.clone();
        let agents = agents.clone();
        let swarm_task = swarm_task.clone();
        let swarm_name = swarm_name.clone();

        Box::pin(async move {
            let selected_agent = select_agent_for_task(&task, &agents);
            let agent_spec = selected_agent
                .as_ref()
                .and_then(|id| agents.iter().find(|agent| agent.id == *id));

            let mut config = OpenAIConfig::new(api_key);
            if let Some(url) = base_url {
                config = config.with_base_url(url);
            }

            if let Some(model) = agent_spec.and_then(|agent| agent.model.as_deref()) {
                config = config.with_model(model);
            } else if let Some(model) = default_model {
                config = config.with_model(model);
            }

            let provider = OpenAIProvider::with_config(config);
            let mut builder = LLMAgentBuilder::new().with_provider(Arc::new(provider));

            if let Some(agent_id) = selected_agent.clone() {
                builder = builder.with_id(agent_id);
            }

            let system_prompt = build_system_prompt(&swarm_name, &swarm_task, agent_spec);
            builder = builder.with_system_prompt(system_prompt);

            let agent = builder.build();
            // The task prompt carries direct dependency outputs from the scheduler.
            let prompt = build_task_prompt(&task, &context);

            let output = agent
                .ask(prompt)
                .await
                .map_err(|e| mofa_kernel::agent::types::error::GlobalError::Other(e.to_string()))?;

            Ok(output)
        })
    }))
}

fn select_agent_for_task(task: &SwarmSubtask, agents: &[AgentSpec]) -> Option<String> {
    if let Some(agent) = &task.assigned_agent {
        return Some(agent.clone());
    }

    if agents.is_empty() {
        return None;
    }

    if task.required_capabilities.is_empty() {
        return Some(agents[0].id.clone());
    }

    // Prefer the agent that matches the most required capabilities.
    let mut best: Option<(&AgentSpec, usize)> = None;
    for agent in agents {
        let matches = task
            .required_capabilities
            .iter()
            .filter(|cap| agent.capabilities.iter().any(|c| c == *cap))
            .count();
        if matches == 0 {
            continue;
        }
        match best {
            Some((_, best_score)) if matches <= best_score => {}
            _ => best = Some((agent, matches)),
        }
    }

    best.map(|(agent, _)| agent.id.clone())
}

fn build_system_prompt(
    swarm_name: &str,
    swarm_task: &str,
    agent_spec: Option<&AgentSpec>,
) -> String {
    let mut prompt = String::new();
    prompt.push_str("You are a swarm subtask agent.\n");
    prompt.push_str(&format!("Swarm: {swarm_name}\n"));
    prompt.push_str(&format!("Overall task: {swarm_task}\n"));

    if let Some(agent) = agent_spec {
        if !agent.capabilities.is_empty() {
            prompt.push_str(&format!(
                "Capabilities: {}\n",
                agent.capabilities.join(", ")
            ));
        }
        if let Some(model) = &agent.model {
            prompt.push_str(&format!("Model: {model}\n"));
        }
    }

    prompt.push_str("Return a concise, actionable result for the subtask.");
    prompt
}

fn build_task_prompt(task: &SwarmSubtask, context: &TaskExecutionContext) -> String {
    let mut prompt = String::new();
    prompt.push_str("Subtask description:\n");
    prompt.push_str(&task.description);
    if !task.required_capabilities.is_empty() {
        prompt.push_str("\n\nRequired capabilities:\n");
        prompt.push_str(&task.required_capabilities.join(", "));
    }
    if !context.dependencies.is_empty() {
        // Only direct dependency outputs are injected here; global run state stays out of prompt assembly.
        prompt.push_str("\n\nUpstream task outputs:\n");
        for dependency in &context.dependencies {
            prompt.push_str("- ");
            prompt.push_str(&dependency.task_id);
            prompt.push_str(": ");
            prompt.push_str(
                dependency
                    .output
                    .as_deref()
                    .unwrap_or("(completed with no output)"),
            );
            prompt.push('\n');
        }
    }
    prompt
}

/// Print a colourful summary table after the run.
fn print_summary(summary: &SchedulerSummary, wall_ms: u128) {
    println!();
    let status_str = if summary.failed == 0 {
        "SUCCESS".green().bold()
    } else {
        "PARTIAL FAILURE".red().bold()
    };

    println!("  {} {}", "◈ Swarm complete:".bold(), status_str);
    println!("  ─────────────────────────────");
    println!(
        "  {}   {}/{}",
        "Tasks:".dimmed(),
        summary.succeeded.to_string().green(),
        summary.total_tasks.to_string().bold()
    );
    if summary.failed > 0 {
        println!(
            "  {}  {}",
            "Failed:".dimmed(),
            summary.failed.to_string().red()
        );
    }
    if summary.skipped > 0 {
        println!(
            "  {}  {}",
            "Skipped:".dimmed(),
            summary.skipped.to_string().yellow()
        );
    }
    println!(
        "  {} {}ms",
        "Wall time:".dimmed(),
        wall_ms.to_string().cyan()
    );

    // Print each task result
    println!();
    for result in &summary.results {
        let icon = if result.outcome.is_success() {
            "✓".green()
        } else {
            "✗".red()
        };
        let ms = result.wall_time.as_millis();
        let outcome = match &result.outcome {
            mofa_foundation::swarm::TaskOutcome::Success(output) => {
                format!("  {}", truncate(output, 80).dimmed())
            }
            mofa_foundation::swarm::TaskOutcome::Failure(err) => {
                format!("  {}", truncate(err, 80).red())
            }
            mofa_foundation::swarm::TaskOutcome::Skipped(reason) => {
                format!("  {}", truncate(reason, 80).yellow())
            }
        };
        println!(
            "    {} {} ({}ms){}",
            icon,
            result.task_id.bold(),
            ms.to_string().dimmed(),
            outcome
        );
    }
    println!();
}

fn truncate(value: &str, max: usize) -> String {
    let mut out = String::new();
    let mut count = 0usize;
    for ch in value.chars() {
        if count >= max {
            out.push('…');
            return out;
        }
        out.push(ch);
        count += 1;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use mofa_foundation::swarm::TaskDependencyContext;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[tokio::test]
    async fn test_run_swarm_requires_llm_credentials() {
        let yaml = r#"
name: test-swarm
description: "A simple test swarm"
task: "Run a test workflow"
pattern: sequential
executor: llm
agents:
  - id: step-a
    capabilities: [search]
    model: llama-3.1-8b-instant
  - id: step-b
    capabilities: [analyze]
    model: llama-3.1-8b-instant
dag:
  tasks:
    - id: step-a
      description: "Search for information"
      capabilities: [search]
    - id: step-b
      description: "Analyze the findings"
      capabilities: [analyze]
  dependencies:
    - from: step-a
      to: step-b
"#;
        let mut tmp = NamedTempFile::new().unwrap();
        tmp.write_all(yaml.as_bytes()).unwrap();

        let result = run_swarm(tmp.path(), false).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_run_swarm_missing_dag_returns_error() {
        let yaml = r#"
name: generic-swarm
task: "Do something interesting"
executor: llm
"#;
        let mut tmp = NamedTempFile::new().unwrap();
        tmp.write_all(yaml.as_bytes()).unwrap();

        let result = run_swarm(tmp.path(), false).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_run_swarm_bad_file_returns_error() {
        let result = run_swarm(Path::new("/nonexistent/path.yaml"), false).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_run_swarm_invalid_yaml_returns_error() {
        let mut tmp = NamedTempFile::new().unwrap();
        tmp.write_all(b"not: valid: yaml: :::").unwrap();
        let result = run_swarm(tmp.path(), false).await;
        assert!(result.is_err());
    }

    #[test]
    fn test_swarm_run_file_parses_llm_executor_and_dag() {
        let yaml = r#"
name: llm-swarm
task: "Research a topic"
pattern: parallel
executor: llm
agents:
  - id: researcher
    capabilities: [web_search]
    model: llama-3.1-8b-instant
dag:
  tasks:
    - id: search
      description: "Search for sources"
      capabilities: [web_search]
"#;

        let run_file: SwarmRunFile = serde_yaml::from_str(yaml).unwrap();
        assert!(matches!(run_file.executor, SwarmExecutorKind::Llm));
        let dag = run_file.dag.expect("dag should parse");
        assert_eq!(dag.task_count(), 1);
        let search = dag.find_by_id("search").expect("task should exist");
        assert_eq!(dag.get_task(search).unwrap().id, "search");
        assert_eq!(
            dag.get_task(search).unwrap().required_capabilities,
            vec!["web_search"]
        );
    }

    #[test]
    fn test_build_task_prompt_includes_dependency_outputs() {
        let task = SwarmSubtask::new("summarize", "Write the final summary")
            .with_capabilities(vec!["summarize".into(), "write".into()]);
        let context = TaskExecutionContext {
            dependencies: vec![
                TaskDependencyContext {
                    task_id: "analyze".into(),
                    output: Some("Ranked breakthroughs: A, B, C".into()),
                },
                TaskDependencyContext {
                    task_id: "collect_stats".into(),
                    output: Some("Stats collected for all candidates".into()),
                },
            ],
        };

        let prompt = build_task_prompt(&task, &context);

        assert!(prompt.contains("Subtask description:\nWrite the final summary"));
        assert!(prompt.contains("Required capabilities:\nsummarize, write"));
        assert!(prompt.contains("Upstream task outputs:"));
        assert!(prompt.contains("- analyze: Ranked breakthroughs: A, B, C"));
        assert!(prompt.contains("- collect_stats: Stats collected for all candidates"));
    }
}
