//! `mofa vibe` command implementation.

use crate::CliError;
use dialoguer::{Input, Select};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

const DEFAULT_MODEL: &str = "gpt-4o-mini";
const DEFAULT_OPENAI_BASE_URL: &str = "https://api.openai.com/v1";
const DEFAULT_FLOW_OUTPUT: &str = "dataflow.yml";
const DEFAULT_AGENT_OUTPUT: &str = "agents/generated-agent";

#[derive(Debug, Serialize)]
struct ChatRequest {
    model: String,
    messages: Vec<ChatMessage>,
    temperature: f32,
}

#[derive(Debug, Serialize)]
struct ChatMessage {
    role: String,
    content: String,
}

#[derive(Debug, Deserialize)]
struct ChatResponse {
    choices: Vec<ChatChoice>,
}

#[derive(Debug, Deserialize)]
struct ChatChoice {
    message: ChatChoiceMessage,
}

#[derive(Debug, Deserialize)]
struct ChatChoiceMessage {
    content: String,
}

pub async fn run_tui() -> Result<(), CliError> {
    let selection = Select::new()
        .with_prompt("MoFA Vibe")
        .items(&["Generate flow", "Generate agent", "Quit"])
        .default(0)
        .interact()
        .map_err(CliError::from)?;

    match selection {
        0 => run_flow(None, None, None).await,
        1 => run_agent(None, None, None, None, None).await,
        _ => Ok(()),
    }
}

pub async fn run_flow(
    llm: Option<&str>,
    output: Option<&Path>,
    requirement: Option<&str>,
) -> Result<(), CliError> {
    let model = llm.unwrap_or(DEFAULT_MODEL);
    let output_path = output.unwrap_or_else(|| Path::new(DEFAULT_FLOW_OUTPUT));
    let requirement = resolve_requirement(requirement, "Describe the flow (what it should do)")?;

    println!("Generating vibe flow: {}", output_path.display());
    let prompt = build_flow_prompt(&requirement);
    let generated = generate_via_llm(model, &prompt).await?;
    validate_flow_output(&generated)?;

    std::fs::write(output_path, generated).map_err(|err| {
        CliError::Other(format!(
            "failed to write generated flow to {}: {}",
            output_path.display(),
            err
        ))
    })?;

    println!("Flow generated at {}", output_path.display());
    Ok(())
}

pub async fn run_agent(
    llm: Option<&str>,
    max_rounds: Option<u32>,
    output: Option<&Path>,
    base: Option<&Path>,
    requirement: Option<&str>,
) -> Result<(), CliError> {
    let model = llm.unwrap_or(DEFAULT_MODEL);
    let output_dir = output
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from(DEFAULT_AGENT_OUTPUT));
    let requirement = resolve_requirement(requirement, "Describe the agent (what it should do)")?;

    println!("Generating vibe agent: {}", output_dir.display());
    let prompt = build_agent_prompt(
        &requirement,
        max_rounds,
        base.map(|path| path.display().to_string()),
    );
    let generated = generate_via_llm(model, &prompt).await?;

    std::fs::create_dir_all(&output_dir).map_err(|err| {
        CliError::Other(format!(
            "failed to create output directory {}: {}",
            output_dir.display(),
            err
        ))
    })?;
    let main_py = output_dir.join("main.py");
    std::fs::write(&main_py, generated).map_err(|err| {
        CliError::Other(format!(
            "failed to write generated agent to {}: {}",
            main_py.display(),
            err
        ))
    })?;

    println!("Agent generated at {}", output_dir.display());
    Ok(())
}

fn resolve_requirement(requirement: Option<&str>, prompt: &str) -> Result<String, CliError> {
    let raw = if let Some(value) = requirement {
        value.to_string()
    } else {
        Input::<String>::new()
            .with_prompt(prompt)
            .interact_text()
            .map_err(CliError::from)?
    };

    let normalized = raw.trim().to_string();
    if normalized.is_empty() {
        return Err(CliError::Other("requirement cannot be empty".to_string()));
    }
    Ok(normalized)
}

fn build_flow_prompt(requirement: &str) -> String {
    format!(
        "Generate a MoFA dataflow YAML for this requirement:\n{requirement}\n\
Return only YAML. The YAML must contain `nodes`, use realistic MoFA operators, \
and reflect the requirement rather than a generic scaffold."
    )
}

fn build_agent_prompt(
    requirement: &str,
    max_rounds: Option<u32>,
    base_path: Option<String>,
) -> String {
    let mut prompt = format!(
        "Generate a Python MoFA agent implementation for this requirement:\n{requirement}\n\
Return only Python source code for `main.py`."
    );

    if let Some(rounds) = max_rounds {
        prompt.push_str(&format!("\nMaximum optimization rounds hint: {rounds}."));
    }
    if let Some(base) = base_path {
        prompt.push_str(&format!(
            "\nBase the result on this existing agent path when relevant: {base}."
        ));
    }

    prompt
}

fn validate_flow_output(output: &str) -> Result<(), CliError> {
    if !output.contains("nodes:") {
        return Err(CliError::Other(
            "LLM output does not look like a MoFA flow YAML (`nodes:` missing)".to_string(),
        ));
    }
    Ok(())
}

async fn generate_via_llm(model: &str, prompt: &str) -> Result<String, CliError> {
    let api_key = std::env::var("OPENAI_API_KEY").map_err(|_| {
        CliError::Other("OPENAI_API_KEY is required for `mofa vibe` commands".to_string())
    })?;
    let base_url =
        std::env::var("OPENAI_BASE_URL").unwrap_or_else(|_| DEFAULT_OPENAI_BASE_URL.to_string());
    let endpoint = format!("{}/chat/completions", base_url.trim_end_matches('/'));

    let request = ChatRequest {
        model: model.to_string(),
        messages: vec![
            ChatMessage {
                role: "system".to_string(),
                content: "You generate MoFA artifacts. Return only the requested artifact with no markdown fencing.".to_string(),
            },
            ChatMessage {
                role: "user".to_string(),
                content: prompt.to_string(),
            },
        ],
        temperature: 0.2,
    };

    let response = reqwest::Client::new()
        .post(endpoint)
        .bearer_auth(api_key)
        .json(&request)
        .send()
        .await
        .map_err(|err| CliError::ApiError(format!("OpenAI request failed: {err}")))?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response
            .text()
            .await
            .unwrap_or_else(|_| "<unreadable response body>".to_string());
        return Err(CliError::ApiError(format!(
            "OpenAI API error {status}: {body}"
        )));
    }

    let parsed: ChatResponse = response
        .json()
        .await
        .map_err(|err| CliError::ApiError(format!("failed to parse OpenAI response: {err}")))?;

    parsed
        .choices
        .into_iter()
        .next()
        .map(|choice| choice.message.content.trim().to_string())
        .filter(|content| !content.is_empty())
        .ok_or_else(|| CliError::ApiError("OpenAI response had no generated content".to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_flow_prompt_includes_requirement_and_yaml_contract() {
        let prompt = build_flow_prompt("build a multilingual translation workflow");
        assert!(prompt.contains("multilingual translation workflow"));
        assert!(prompt.contains("Return only YAML"));
        assert!(prompt.contains("reflect the requirement"));
    }

    #[test]
    fn build_agent_prompt_includes_requirement_and_options() {
        let prompt = build_agent_prompt(
            "build a support triage agent",
            Some(4),
            Some("agents/base-agent".to_string()),
        );
        assert!(prompt.contains("support triage agent"));
        assert!(prompt.contains("Maximum optimization rounds hint: 4"));
        assert!(prompt.contains("agents/base-agent"));
    }

    #[test]
    fn validate_flow_output_rejects_non_flow_content() {
        let err = validate_flow_output("name: not-a-flow").unwrap_err();
        assert!(err.to_string().contains("nodes:` missing"));
    }

    #[test]
    fn resolve_requirement_rejects_empty_input() {
        let err = resolve_requirement(Some("  "), "ignored").unwrap_err();
        assert!(err.to_string().contains("requirement cannot be empty"));
    }
}
