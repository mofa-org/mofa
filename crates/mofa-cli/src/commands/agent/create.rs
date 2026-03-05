//! `mofa agent create` command - Interactive agent creation wizard

use crate::CliError;
use colored::Colorize;
use dialoguer::{theme::ColorfulTheme, Confirm, Input, Select};
use serde::Deserialize;
use serde::Serialize;
use std::path::PathBuf;

/// Agent configuration built from the interactive wizard
#[derive(Debug, Clone)]
pub struct AgentConfigBuilder {
    pub id: String,
    pub name: String,
    pub description: String,
    pub provider: LLMProvider,
    pub model: String,
    pub api_key: Option<String>,
    pub base_url: Option<String>,
    pub temperature: f32,
    pub max_tokens: u32,
    pub system_prompt: String,
    pub capabilities: Vec<String>,
}

/// Available LLM providers
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LLMProvider {
    OpenAI,
    Ollama,
    Azure,
    Compatible,
    Anthropic,
    Gemini,
}

impl LLMProvider {
    fn name(&self) -> &str {
        match self {
            Self::OpenAI => "OpenAI",
            Self::Ollama => "Ollama",
            Self::Azure => "Azure OpenAI",
            Self::Compatible => "Compatible API",
            Self::Anthropic => "Anthropic Claude",
            Self::Gemini => "Google Gemini",
        }
    }

    fn default_model(&self) -> &str {
        match self {
            Self::OpenAI => "gpt-4o",
            Self::Ollama => "llama2",
            Self::Azure => "gpt-4o",
            Self::Compatible => "gpt-4o",
            Self::Anthropic => "claude-3.5-sonnet-20241022",
            Self::Gemini => "gemini-1.5-pro-latest",
        }
    }

    fn needs_api_key(&self) -> bool {
        !matches!(self, Self::Ollama)
    }
}

/// Execute the `mofa agent create` command
pub fn run(non_interactive: bool, config_path: Option<PathBuf>) -> Result<(), CliError> {
    if non_interactive {
        // Non-interactive mode - use config file or defaults
        let config = config_from_file_or_defaults(config_path)?;
        write_agent_config(&config)?;
    } else {
        // Interactive mode - run the wizard
        let config = run_interactive_wizard()?;
        write_agent_config(&config)?;
    }

    Ok(())
}

fn run_interactive_wizard() -> Result<AgentConfigBuilder, CliError> {
    println!();
    println!(
        "{}",
        "═══════════════════════════════════════════════".cyan()
    );
    println!("{}", "  MoFA Agent Creation Wizard".cyan().bold());
    println!(
        "{}",
        "═══════════════════════════════════════════════".cyan()
    );
    println!();

    let theme = ColorfulTheme::default();

    // Step 1: Agent Identity
    println!("{}", "Step 1: Agent Identity".bold().yellow());
    let name: String = Input::with_theme(&theme)
        .with_prompt("Agent name")
        .default("MyAgent".to_string())
        .interact()?;

    let id = Input::with_theme(&theme)
        .with_prompt("Agent ID")
        .default(format!("{}-001", name.to_lowercase().replace(' ', "-")))
        .interact()?;

    let description: String = Input::with_theme(&theme)
        .with_prompt("Agent description")
        .default("A helpful AI assistant".to_string())
        .interact()?;

    println!();

    // Step 2: LLM Provider
    println!("{}", "Step 2: LLM Provider".bold().yellow());
    let providers = vec![
        "OpenAI",
        "Ollama",
        "Azure OpenAI",
        "Compatible API",
        "Anthropic Claude",
        "Google Gemini",
    ];
    let provider_selection = Select::with_theme(&theme)
        .with_prompt("Select LLM provider")
        .items(&providers)
        .default(0)
        .interact()?;

    let provider = match provider_selection {
        0 => LLMProvider::OpenAI,
        1 => LLMProvider::Ollama,
        2 => LLMProvider::Azure,
        3 => LLMProvider::Compatible,
        4 => LLMProvider::Anthropic,
        5 => LLMProvider::Gemini,
        _ => LLMProvider::OpenAI,
    };

    println!();

    // Step 3: Model Configuration
    println!("{}", "Step 3: Model Configuration".bold().yellow());
    let model = Input::with_theme(&theme)
        .with_prompt("Model name")
        .default(provider.default_model().to_string())
        .interact()?;

    let api_key = if provider.needs_api_key() {
        let key: String = Input::with_theme(&theme)
            .with_prompt("API key (or leave empty to use env var)")
            .allow_empty(true)
            .interact()?;
        if key.is_empty() {
            None
        } else {
            Some(key)
        }
    } else {
        None
    };

    let base_url: Option<String> = if !matches!(provider, LLMProvider::OpenAI | LLMProvider::Ollama)
    {
        let url: String = Input::with_theme(&theme)
            .with_prompt("Base URL (optional)")
            .allow_empty(true)
            .interact()?;
        if url.is_empty() {
            None
        } else {
            Some(url)
        }
    } else {
        None
    };

    println!();

    // Step 4: Generation Parameters
    println!("{}", "Step 4: Generation Parameters".bold().yellow());
    let temperature: f32 = Input::with_theme(&theme)
        .with_prompt("Temperature (0.0 - 2.0)")
        .default("0.7".to_string())
        .validate_with(|input: &String| {
            let temp = input.parse::<f32>().map_err(|_| "Must be a number")?;
            if (0.0..=2.0).contains(&temp) {
                Ok(())
            } else {
                Err("Must be between 0.0 and 2.0")
            }
        })
        .interact()?
        .parse()?;

    let max_tokens: u32 = Input::with_theme(&theme)
        .with_prompt("Max tokens")
        .default("4096".to_string())
        .validate_with(|input: &String| {
            input
                .parse::<u32>()
                .map_err(|_| "Must be a positive number")
                .map(|_| ())
        })
        .interact()?
        .parse()?;

    println!();

    // Step 5: System Prompt
    println!("{}", "Step 5: System Prompt".bold().yellow());
    println!("Define your agent's personality and behavior:");
    println!("(Press Enter twice to finish input)");
    println!();

    let system_prompt = multiline_input(
        "You are a helpful AI assistant. Be concise, accurate, and friendly.\n\
         When you don't know something, say so rather than making up information.",
    );

    println!();

    // Step 6: Capabilities
    println!("{}", "Step 6: Capabilities".bold().yellow());
    println!("Enter capabilities (comma-separated):");
    println!("  Options: llm, chat, tool_call, memory, storage, workflow");
    println!("  Default: llm, chat");
    println!();

    let capabilities_input: String = Input::with_theme(&theme)
        .with_prompt("Capabilities")
        .default("llm, chat".to_string())
        .allow_empty(true)
        .interact()?;

    let capabilities: Vec<String> = capabilities_input
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();

    println!();

    // Confirmation
    println!(
        "{}",
        "═══════════════════════════════════════════════".cyan()
    );
    println!("{}", "  Configuration Summary".bold());
    println!(
        "{}",
        "═══════════════════════════════════════════════".cyan()
    );
    println!("  Name:           {}", name.cyan());
    println!("  ID:             {}", id.cyan());
    println!("  Description:    {}", description.white());
    println!("  Provider:       {}", provider.name().yellow());
    println!("  Model:          {}", model.yellow());
    println!("  Temperature:    {}", temperature.to_string().white());
    println!("  Max Tokens:     {}", max_tokens.to_string().white());
    println!("  Capabilities:   {}", capabilities.join(", ").cyan());
    println!();

    let confirmed = Confirm::with_theme(&theme)
        .with_prompt("Create this agent?")
        .default(true)
        .interact()?;

    if !confirmed {
        println!("{}", "Agent creation cancelled.".red());
        std::process::exit(0);
    }

    println!();

    Ok(AgentConfigBuilder {
        id,
        name,
        description,
        provider,
        model,
        api_key,
        base_url,
        temperature,
        max_tokens,
        system_prompt,
        capabilities,
    })
}

fn config_from_file_or_defaults(
    config_path: Option<PathBuf>,
) -> Result<AgentConfigBuilder, CliError> {
    if let Some(path) = config_path {
        #[derive(Debug, Deserialize)]
        struct FileAgentConfig {
            id: String,
            name: String,
            #[serde(default)]
            description: String,
            #[serde(default)]
            capabilities: Vec<String>,
        }

        #[derive(Debug, Deserialize)]
        struct FileLlmConfig {
            provider: Option<String>,
            model: Option<String>,
            api_key: Option<String>,
            base_url: Option<String>,
            temperature: Option<f32>,
            max_tokens: Option<u32>,
            #[serde(default)]
            system_prompt: String,
        }

        #[derive(Debug, Deserialize)]
        struct FileRootConfig {
            agent: FileAgentConfig,
            llm: FileLlmConfig,
        }

        let raw = std::fs::read_to_string(&path).map_err(|e| {
            CliError::ConfigError(format!(
                "Failed to read config file {}: {e}",
                path.display()
            ))
        })?;

        let parsed: FileRootConfig = serde_yaml::from_str(&raw).map_err(|e| {
            CliError::ConfigError(format!(
                "Failed to parse config file {}: {e}",
                path.display()
            ))
        })?;

        let provider_value = parsed.llm.provider.ok_or_else(|| {
            CliError::ConfigError("Missing required config field: llm.provider".into())
        })?;

        let provider = parse_provider(&provider_value)?;

        let model = parsed
            .llm
            .model
            .unwrap_or_else(|| provider.default_model().to_string());

        let capabilities = if parsed.agent.capabilities.is_empty() {
            vec!["llm".to_string()]
        } else {
            parsed.agent.capabilities
        };

        Ok(AgentConfigBuilder {
            id: parsed.agent.id,
            name: parsed.agent.name,
            description: parsed.agent.description,
            provider,
            model,
            api_key: parsed.llm.api_key,
            base_url: parsed.llm.base_url,
            temperature: parsed.llm.temperature.unwrap_or(0.7),
            max_tokens: parsed.llm.max_tokens.unwrap_or(4096),
            system_prompt: parsed.llm.system_prompt,
            capabilities,
        })
    } else {
        // Use defaults
        Ok(AgentConfigBuilder {
            id: "agent-001".to_string(),
            name: "MyAgent".to_string(),
            description: "A helpful AI assistant".to_string(),
            provider: LLMProvider::OpenAI,
            model: "gpt-4o".to_string(),
            api_key: None,
            base_url: None,
            temperature: 0.7,
            max_tokens: 4096,
            system_prompt: "You are a helpful AI assistant.".to_string(),
            capabilities: vec!["llm".to_string(), "chat".to_string()],
        })
    }
}

fn parse_provider(value: &str) -> Result<LLMProvider, CliError> {
    match value.trim().to_ascii_lowercase().as_str() {
        "openai" => Ok(LLMProvider::OpenAI),
        "ollama" => Ok(LLMProvider::Ollama),
        "azure" | "azure_openai" | "azure-openai" => Ok(LLMProvider::Azure),
        "compatible" | "compatible_api" | "compatible-api" => Ok(LLMProvider::Compatible),
        "anthropic" => Ok(LLMProvider::Anthropic),
        "gemini" => Ok(LLMProvider::Gemini),
        other => {
            return Err(CliError::ConfigError(format!(
                "Unsupported llm.provider value: {other}"
            )))
        }
    }
}

fn write_agent_config(config: &AgentConfigBuilder) -> Result<(), CliError> {
    let filename = "agent.yml";

    let content = build_agent_config_yaml(config)?;

    std::fs::write(filename, content)?;

    println!(
        "{} Agent configuration written to {}",
        "✓".green(),
        filename.cyan()
    );
    println!();
    println!("Next steps:");
    println!(
        "  1. Review and edit {} to customize your agent",
        filename.cyan()
    );
    println!("  2. Set your API key: export OPENAI_API_KEY='sk-...'");
    println!("  3. Run your agent: mofa run");
    println!();

    Ok(())
}

fn build_agent_config_yaml(config: &AgentConfigBuilder) -> Result<String, CliError> {
    #[derive(Serialize)]
    struct AgentSection<'a> {
        id: &'a str,
        name: &'a str,
        description: &'a str,
        capabilities: Vec<String>,
    }

    #[derive(Serialize)]
    struct LlmSection<'a> {
        provider: &'a str,
        model: &'a str,
        api_key: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        base_url: Option<&'a str>,
        temperature: f32,
        max_tokens: u32,
        system_prompt: &'a str,
    }

    #[derive(Serialize)]
    struct RuntimeSection {
        max_concurrent_tasks: u32,
        default_timeout_secs: u32,
    }

    #[derive(Serialize)]
    struct AgentFileConfig<'a> {
        agent: AgentSection<'a>,
        llm: LlmSection<'a>,
        runtime: RuntimeSection,
    }

    let provider_name = match config.provider {
        LLMProvider::OpenAI => "openai",
        LLMProvider::Ollama => "ollama",
        LLMProvider::Azure => "azure",
        LLMProvider::Compatible => "compatible",
        LLMProvider::Anthropic => "anthropic",
        LLMProvider::Gemini => "gemini",
    };

    let api_key = match &config.api_key {
        Some(key) => key.clone(),
        None => match config.provider {
            LLMProvider::OpenAI | LLMProvider::Azure | LLMProvider::Compatible => {
                "${OPENAI_API_KEY}".to_string()
            }
            LLMProvider::Anthropic => "${ANTHROPIC_API_KEY}".to_string(),
            LLMProvider::Gemini => "${GEMINI_API_KEY}".to_string(),
            LLMProvider::Ollama => String::new(),
        },
    };

    let capabilities = if config.capabilities.is_empty() {
        vec!["llm".to_string()]
    } else {
        config.capabilities.clone()
    };

    let file = AgentFileConfig {
        agent: AgentSection {
            id: &config.id,
            name: &config.name,
            description: &config.description,
            capabilities,
        },
        llm: LlmSection {
            provider: provider_name,
            model: &config.model,
            api_key,
            base_url: config.base_url.as_deref(),
            temperature: config.temperature,
            max_tokens: config.max_tokens,
            system_prompt: &config.system_prompt,
        },
        runtime: RuntimeSection {
            max_concurrent_tasks: 10,
            default_timeout_secs: 60,
        },
    };

    let mut yaml = serde_yaml::to_string(&file).map_err(|e| {
        CliError::ConfigError(format!("Failed to serialize agent config YAML: {e}"))
    })?;
    if !yaml.ends_with('\n') {
        yaml.push('\n');
    }

    Ok(format!(
        "# MoFA Agent Configuration\n# Generated by mofa agent create\n\n{}",
        yaml
    ))
}

/// Helper for multiline input
fn multiline_input(default: &str) -> String {
    // For now, just use default value
    // In a full implementation, this would open a proper multiline editor
    default.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn write_config_file(temp: &TempDir, content: &str) -> PathBuf {
        let path = temp.path().join("agent.yml");
        fs::write(&path, content).expect("failed to write temp config");
        path
    }

    #[test]
    fn config_path_loads_into_builder_flow() {
        let temp = TempDir::new().expect("failed to create tempdir");
        let config_path = write_config_file(
            &temp,
            r#"
agent:
  id: "support-agent-001"
  name: "Support Agent"
  description: "Handles support requests"
  capabilities:
    - llm
    - tool_call
llm:
  provider: anthropic
  model: claude-3-5-sonnet-20241022
  api_key: ${ANTHROPIC_API_KEY}
  base_url: https://api.anthropic.com
  temperature: 0.35
  max_tokens: 2048
  system_prompt: |
    You are a support assistant.
"#,
        );

        let config = config_from_file_or_defaults(Some(config_path)).expect("expected config");
        assert_eq!(config.id, "support-agent-001");
        assert_eq!(config.name, "Support Agent");
        assert_eq!(config.description, "Handles support requests");
        assert_eq!(config.provider, LLMProvider::Anthropic);
        assert_eq!(config.model, "claude-3-5-sonnet-20241022");
        assert_eq!(config.api_key.as_deref(), Some("${ANTHROPIC_API_KEY}"));
        assert_eq!(
            config.base_url.as_deref(),
            Some("https://api.anthropic.com")
        );
        assert_eq!(config.temperature, 0.35);
        assert_eq!(config.max_tokens, 2048);
        assert_eq!(config.system_prompt.trim(), "You are a support assistant.");
        assert_eq!(config.capabilities, vec!["llm", "tool_call"]);
    }

    #[test]
    fn malformed_config_returns_clear_error() {
        let temp = TempDir::new().expect("failed to create tempdir");
        let config_path = write_config_file(
            &temp,
            r#"
agent:
  id: "bad-agent"
  name: "Bad Agent"
llm: [this is not valid
"#,
        );

        let err =
            config_from_file_or_defaults(Some(config_path)).expect_err("expected parse error");
        let msg = err.to_string();
        assert!(msg.contains("Failed to parse config file"));
    }

    #[test]
    fn missing_required_fields_returns_useful_message() {
        let temp = TempDir::new().expect("failed to create tempdir");
        let config_path = write_config_file(
            &temp,
            r#"
agent:
  id: "missing-provider-agent"
  name: "Missing Provider"
llm:
  model: gpt-4o
"#,
        );

        let err = config_from_file_or_defaults(Some(config_path))
            .expect_err("expected missing-required-field error");
        let msg = err.to_string();
        assert!(msg.contains("Missing required config field"));
        assert!(msg.contains("llm.provider"));
    }
}
