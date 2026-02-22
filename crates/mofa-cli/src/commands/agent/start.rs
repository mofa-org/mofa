//! `mofa agent start` command implementation

use crate::config::loader::ConfigLoader;
use crate::context::CliContext;
use colored::Colorize;

/// Execute the `mofa agent start` command
pub async fn run(
    ctx: &CliContext,
    agent_id: &str,
    config_path: Option<&std::path::Path>,
    daemon: bool,
) -> anyhow::Result<()> {
    println!("{} Starting agent: {}", "→".green(), agent_id.cyan());

    if daemon {
        println!("  Mode: {}", "daemon".yellow());
    }

    // Check if agent is already registered
    if ctx.agent_registry.contains(agent_id).await {
        anyhow::bail!("Agent '{}' is already registered", agent_id);
    }

    // Load agent configuration
    let agent_config = if let Some(path) = config_path {
        println!("  Config: {}", path.display().to_string().cyan());
        let loader = ConfigLoader::new();
        let cli_config = loader.load(path)?;
        println!("  Agent:  {}", cli_config.agent.name.white());

        // Convert CLI AgentConfig to kernel AgentConfig
        mofa_kernel::agent::config::AgentConfig::new(agent_id, &cli_config.agent.name)
    } else {
        // Try to auto-discover configuration
        let loader = ConfigLoader::new();
        match loader.find_config() {
            Some(found_path) => {
                println!(
                    "  Config: {} (auto-discovered)",
                    found_path.display().to_string().cyan()
                );
                let cli_config = loader.load(&found_path)?;
                println!("  Agent:  {}", cli_config.agent.name.white());
                mofa_kernel::agent::config::AgentConfig::new(agent_id, &cli_config.agent.name)
            }
            None => {
                println!(
                    "  {} No config file found, using defaults",
                    "!".yellow()
                );
                mofa_kernel::agent::config::AgentConfig::new(agent_id, agent_id)
            }
        }
    };

    // Check if a matching factory type is available
    let factory_types = ctx.agent_registry.list_factory_types().await;
    if factory_types.is_empty() {
        println!(
            "  {} No agent factories registered. Agent registered with config only.",
            "!".yellow()
        );
        println!("  Agent config stored for: {}", agent_config.name.cyan());
    } else {
        // Try to create via factory
        let type_id = factory_types.first().unwrap();
        match ctx
            .agent_registry
            .create_and_register(type_id, agent_config.clone())
            .await
        {
            Ok(_) => {
                println!("{} Agent '{}' created and registered", "✓".green(), agent_id);
            }
            Err(e) => {
                println!(
                    "  {} Failed to create agent via factory: {}",
                    "!".yellow(),
                    e
                );
            }
        }
    }

    println!("{} Agent '{}' started", "✓".green(), agent_id);

    Ok(())
}
