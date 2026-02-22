//! `mofa agent start` command implementation

use crate::config::loader::ConfigLoader;
use crate::context::CliContext;
use crate::state::AgentMetadata;
use colored::Colorize;
use std::path::Path;
use tracing::info;

/// Execute the `mofa agent start` command
pub async fn run(
    ctx: &CliContext,
    agent_id: &str,
    config_path: Option<&Path>,
    daemon: bool,
) -> anyhow::Result<()> {
    println!("{} Starting agent: {}", "→".green(), agent_id.cyan());

    if daemon {
        println!("  Mode: {}", "daemon".yellow());
    }

    // Check if agent already exists
    if ctx.persistent_agents.exists(agent_id).await {
        let existing = ctx.persistent_agents.get(agent_id).await;
        if let Some(agent) = existing {
            if agent.last_state == crate::state::AgentProcessState::Running {
                anyhow::bail!(
                    "Agent '{}' is already running (PID: {})",
                    agent_id,
                    agent.process_id.unwrap_or(0)
                );
            }
            println!(
                "  {} Agent exists but is not running. Restarting...",
                "!".yellow()
            );
        }
    }

    // Load or discover agent configuration
    let (config_file, agent_name) = if let Some(path) = config_path {
        println!("  Config: {}", path.display().to_string().cyan());
        ctx.process_manager.validate_config(path)?;

        // Try to load name from config
        let name = load_agent_name_from_config(path).unwrap_or_else(|| agent_id.to_string());

        (path.to_path_buf(), name)
    } else {
        // Try to auto-discover configuration
        let loader = ConfigLoader::new();
        match loader.find_config() {
            Some(found_path) => {
                println!(
                    "  Config: {} (auto-discovered)",
                    found_path.display().to_string().cyan()
                );
                ctx.process_manager.validate_config(&found_path)?;

                let name = load_agent_name_from_config(&found_path)
                    .unwrap_or_else(|| agent_id.to_string());

                (found_path, name)
            }
            None => {
                anyhow::bail!(
                    "No configuration found for agent '{}'. Specify with --config or create a mofa.yaml file.",
                    agent_id
                );
            }
        }
    };

    println!("  Agent:  {}", agent_name.white());

    // Start the agent process
    println!("  {} Spawning process...", "→".green());
    let pid = match ctx
        .process_manager
        .start_agent(agent_id, Some(&config_file), daemon)
    {
        Ok(p) => {
            println!("  PID:    {}", p.to_string().cyan());
            p
        }
        Err(e) => {
            println!("  {} Failed to start process: {}", "✗".red(), e);
            anyhow::bail!("Failed to start agent process: {}", e);
        }
    };

    // Create and persist agent metadata
    let mut metadata = AgentMetadata::new(agent_id.to_string(), agent_name);
    metadata = metadata.with_config(config_file);
    metadata.mark_started(pid);

    ctx.persistent_agents.register(metadata).await?;

    println!("{} Agent '{}' started successfully", "✓".green(), agent_id);
    println!("  PID: {}", pid.to_string().cyan());

    info!("Agent '{}' started with PID: {}", agent_id, pid);

    Ok(())
}

/// Load agent name from configuration file
fn load_agent_name_from_config(path: &Path) -> Option<String> {
    match std::fs::read_to_string(path) {
        Ok(content) => match serde_yaml::from_str::<serde_yaml::Value>(&content) {
            Ok(doc) => doc
                .get("agent")
                .and_then(|agent| agent.get("name"))
                .and_then(|name| name.as_str())
                .map(|s| s.to_string()),
            Err(_) => None,
        },
        Err(_) => None,
    }
}
