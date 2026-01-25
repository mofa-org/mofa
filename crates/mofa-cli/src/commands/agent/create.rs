//! `mofa agent create` command - Interactive agent creation wizard

use colored::Colorize;
use dialoguer::{theme::ColorfulTheme, Confirm, Input, Select};
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
}

impl LLMProvider {
    fn name(&self) -> &str {
        match self {
            Self::OpenAI => "OpenAI",
            Self::Ollama => "Ollama",
            Self::Azure => "Azure OpenAI",
            Self::Compatible => "Compatible API",
        }
    }

    fn default_model(&self) -> &str {
        match self {
            Self::OpenAI => "gpt-4o",
            Self::Ollama => "llama2",
            Self::Azure => "gpt-4o",
            Self::Compatible => "gpt-4o",
        }
    }

    fn needs_api_key(&self) -> bool {
        !matches!(self, Self::Ollama)
    }
}

/// Execute the `mofa agent create` command
pub fn run(non_interactive: bool, config_path: Option<PathBuf>) -> anyhow::Result<()> {
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

fn run_interactive_wizard() -> anyhow::Result<AgentConfigBuilder> {
    println!();
    println!("{}", "═══════════════════════════════════════════════".cyan());
    println!("{}", "  MoFA Agent Creation Wizard".cyan().bold());
    println!("{}", "═══════════════════════════════════════════════".cyan());
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
    let providers = vec!["OpenAI", "Ollama", "Azure OpenAI", "Compatible API"];
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

    let base_url: Option<String> = if provider != LLMProvider::OpenAI {
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
            input.parse::<u32>().map_err(|_| "Must be a positive number").map(|_| ())
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
    println!("{}", "═══════════════════════════════════════════════".cyan());
    println!("{}", "  Configuration Summary".bold());
    println!("{}", "═══════════════════════════════════════════════".cyan());
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

fn config_from_file_or_defaults(config_path: Option<PathBuf>) -> anyhow::Result<AgentConfigBuilder> {
    if let Some(_path) = config_path {
        // Load from file (would parse the file here)
        anyhow::bail!("Config file loading not yet implemented for non-interactive mode");
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

fn write_agent_config(config: &AgentConfigBuilder) -> anyhow::Result<()> {
    let filename = "agent.yml";

    let provider_name = match config.provider {
        LLMProvider::OpenAI => "openai",
        LLMProvider::Ollama => "ollama",
        LLMProvider::Azure => "azure",
        LLMProvider::Compatible => "compatible",
    };

    let capabilities_str = if config.capabilities.is_empty() {
        "    - llm".to_string()
    } else {
        config.capabilities.iter().map(|c| format!("    - {}", c)).collect::<Vec<_>>().join("\n")
    };

    let api_key_str = match &config.api_key {
        Some(key) => format!("  api_key: {}", key),
        None => "  api_key: ${OPENAI_API_KEY}".to_string(),
    };

    let base_url_str = match &config.base_url {
        Some(url) => format!("  base_url: {}", url),
        None => "".to_string(),
    };

    let content = format!(
        r#"# MoFA Agent Configuration
# Generated by mofa agent create

agent:
  id: "{id}"
  name: "{name}"
  description: "{description}"
  capabilities:
{capabilities}

# LLM Provider Configuration
llm:
  provider: {provider}
  model: {model}
{api_key}
{base_url}
  # Generation parameters
  temperature: {temp}
  max_tokens: {max_tokens}

  # System prompt
  system_prompt: |
    {system_prompt}

# Runtime configuration
runtime:
  max_concurrent_tasks: 10
  default_timeout_secs: 60
"#,
        id = config.id,
        name = config.name,
        description = config.description,
        capabilities = capabilities_str,
        provider = provider_name,
        model = config.model,
        api_key = api_key_str,
        base_url = base_url_str,
        temp = config.temperature,
        max_tokens = config.max_tokens,
        system_prompt = config.system_prompt,
    );

    std::fs::write(filename, content)?;

    println!("{} Agent configuration written to {}", "✓".green(), filename.cyan());
    println!();
    println!("Next steps:");
    println!("  1. Review and edit {} to customize your agent", filename.cyan());
    println!("  2. Set your API key: export OPENAI_API_KEY='sk-...'");
    println!("  3. Run your agent: mofa run");
    println!();

    Ok(())
}

/// Helper for multiline input
fn multiline_input(default: &str) -> String {
    // For now, just use default value
    // In a full implementation, this would open a proper multiline editor
    default.to_string()
}
