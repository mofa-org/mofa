//! `mofa vibe` command implementation

use crate::CliError;
use colored::Colorize;
use dialoguer::Input;
use serde::Serialize;
use std::path::Path;

const DEFAULT_MODEL: &str = "gpt-4o-mini";
const DEFAULT_OUTPUT: &str = "dataflow.yml";

#[derive(Serialize)]
struct VibeMeta<'a> {
    model: &'a str,
    requirement: &'a str,
}

#[derive(Serialize)]
struct OperatorSpec<'a> {
    python: &'a str,
}

#[derive(Serialize)]
struct NodeSpec<'a> {
    id: &'a str,
    operator: OperatorSpec<'a>,
    inputs: std::collections::BTreeMap<&'a str, &'a str>,
    outputs: Vec<&'a str>,
}

#[derive(Serialize)]
struct FlowTemplate<'a> {
    vibe: VibeMeta<'a>,
    nodes: Vec<NodeSpec<'a>>,
}

/// Execute `mofa vibe flow`
pub fn run_flow(
    llm: Option<&str>,
    output: Option<&Path>,
    requirement: Option<&str>,
) -> Result<(), CliError> {
    let model = llm.unwrap_or(DEFAULT_MODEL);
    let output_path = output.unwrap_or_else(|| Path::new(DEFAULT_OUTPUT));
    let requirement = resolve_requirement(requirement)?;

    println!(
        "{} Generating vibe flow: {}",
        "→".green(),
        output_path.display()
    );

    let yaml = render_flow_yaml(&requirement, model)?;
    std::fs::write(output_path, yaml).map_err(|err| {
        CliError::Other(format!(
            "failed to write generated flow to {}: {}",
            output_path.display(),
            err
        ))
    })?;

    println!("{} Flow generated!", "✓".green());
    println!("  Requirement: {}", requirement);
    println!("  Model: {}", model);
    println!("  Next: mofa dataflow {}", output_path.display());
    Ok(())
}

fn resolve_requirement(requirement: Option<&str>) -> Result<String, CliError> {
    let raw = if let Some(value) = requirement {
        value.to_string()
    } else {
        Input::<String>::new()
            .with_prompt("Describe the flow (what it should do)")
            .interact_text()
            .map_err(|err| CliError::Other(format!("failed to read requirement input: {}", err)))?
    };

    let normalized = raw.trim().to_string();
    if normalized.is_empty() {
        return Err(CliError::Other("requirement cannot be empty".to_string()));
    }
    Ok(normalized)
}

fn render_flow_yaml(requirement: &str, model: &str) -> Result<String, CliError> {
    let mut source_inputs = std::collections::BTreeMap::new();
    source_inputs.insert("task_input", "source/output");

    let mut worker_inputs = std::collections::BTreeMap::new();
    worker_inputs.insert("task_input", "agent-1/task_output");

    let template = FlowTemplate {
        vibe: VibeMeta { model, requirement },
        nodes: vec![
            NodeSpec {
                id: "agent-1",
                operator: OperatorSpec {
                    python: "agents/agent.py",
                },
                inputs: source_inputs,
                outputs: vec!["task_output"],
            },
            NodeSpec {
                id: "agent-2",
                operator: OperatorSpec {
                    python: "agents/worker.py",
                },
                inputs: worker_inputs,
                outputs: vec!["result"],
            },
        ],
    };

    serde_yaml::to_string(&template)
        .map_err(|err| CliError::Other(format!("failed to serialize flow template: {}", err)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn render_flow_yaml_includes_metadata() {
        let yaml = render_flow_yaml("Build a translation pipeline", "gpt-4o-mini").unwrap();
        assert!(yaml.contains("requirement: Build a translation pipeline"));
        assert!(yaml.contains("model: gpt-4o-mini"));
        assert!(yaml.contains("id: agent-1"));
        assert!(yaml.contains("id: agent-2"));
    }

    #[test]
    fn run_flow_writes_yaml_to_output_file() {
        let dir = tempdir().unwrap();
        let output = dir.path().join("vibe-flow.yml");

        run_flow(
            Some("gpt-4o-mini"),
            Some(&output),
            Some("Summarize and classify user messages"),
        )
        .unwrap();

        let content = std::fs::read_to_string(output).unwrap();
        assert!(content.contains("Summarize and classify user messages"));
        assert!(content.contains("vibe:"));
        assert!(content.contains("nodes:"));
    }

    #[test]
    fn run_flow_rejects_empty_requirement() {
        let dir = tempdir().unwrap();
        let output = dir.path().join("vibe-flow.yml");

        let err = run_flow(Some("gpt-4o-mini"), Some(&output), Some("   ")).unwrap_err();
        assert!(err.to_string().contains("requirement cannot be empty"));
    }
}
