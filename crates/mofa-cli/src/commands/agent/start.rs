//! `mofa agent start` command implementation

use crate::config::loader::ConfigLoader;
use crate::context::{AgentConfigEntry, CliContext};
use colored::Colorize;

/// Execute the `mofa agent start` command
pub async fn run(
    ctx: &CliContext,
    agent_id: &str,
    config_path: Option<&std::path::Path>,
    factory_type: Option<&str>,
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
                println!("  {} No config file found, using defaults", "!".yellow());
                mofa_kernel::agent::config::AgentConfig::new(agent_id, agent_id)
            }
        }
    };

    // Check if a matching factory type is available
    let mut factory_types = ctx.agent_registry.list_factory_types().await;
    if factory_types.is_empty() {
        anyhow::bail!(
            "No agent factories registered. Cannot start agent '{}'",
            agent_id
        );
    }
    factory_types.sort();

    let selected_factory = select_factory_type(&factory_types, factory_type)?;
    println!("  Factory: {}", selected_factory.cyan());
    if factory_type.is_none() && factory_types.len() > 1 {
        println!(
            "  {} Multiple factories available, defaulted to '{}'. Use --type to choose.",
            "!".yellow(),
            selected_factory
        );
    }

    // Try to create via factory
    ctx.agent_registry
        .create_and_register(&selected_factory, agent_config.clone())
        .await
        .map_err(|e| anyhow::anyhow!("Failed to start agent '{}': {}", agent_id, e))?;

    let entry = AgentConfigEntry {
        id: agent_id.to_string(),
        name: agent_config.name.clone(),
        state: "Running".to_string(),
        description: agent_config.description.clone(),
    };
    if let Err(e) = ctx.agent_store.save(agent_id, &entry) {
        let rollback_result = ctx.agent_registry.unregister(agent_id).await;
        match rollback_result {
            Ok(_) => {
                anyhow::bail!(
                    "Failed to persist agent '{}': {}. Rolled back in-memory registration.",
                    agent_id,
                    e
                );
            }
            Err(rollback_err) => {
                anyhow::bail!(
                    "Failed to persist agent '{}': {}. Rollback failed: {}",
                    agent_id,
                    e,
                    rollback_err
                );
            }
        }
    }

    println!("{} Agent '{}' started", "✓".green(), agent_id);

    Ok(())
}

fn select_factory_type(
    factory_types: &[String],
    requested_factory: Option<&str>,
) -> anyhow::Result<String> {
    if let Some(requested) = requested_factory {
        if factory_types.iter().any(|factory| factory == requested) {
            return Ok(requested.to_string());
        }
        anyhow::bail!(
            "Factory '{}' is not registered. Available factories: {}",
            requested,
            factory_types.join(", ")
        );
    }

    Ok(factory_types
        .first()
        .expect("factory_types must be non-empty")
        .clone())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::CliContext;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_start_succeeds_with_default_factory() {
        let temp = TempDir::new().unwrap();
        let ctx = CliContext::with_temp_dir(temp.path()).await.unwrap();

        let result = run(&ctx, "test-agent", None, None, false).await;
        assert!(result.is_ok());
        assert!(ctx.agent_registry.contains("test-agent").await);
    }

    #[tokio::test]
    async fn test_start_returns_err_for_duplicate_agent() {
        let temp = TempDir::new().unwrap();
        let ctx = CliContext::with_temp_dir(temp.path()).await.unwrap();

        run(&ctx, "dup-agent", None, None, false).await.unwrap();
        let result = run(&ctx, "dup-agent", None, None, false).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_start_returns_err_for_unknown_factory_type() {
        let temp = TempDir::new().unwrap();
        let ctx = CliContext::with_temp_dir(temp.path()).await.unwrap();

        let result = run(&ctx, "typed-agent", None, Some("missing-factory"), false).await;
        assert!(result.is_err());
        assert!(!ctx.agent_registry.contains("typed-agent").await);
    }

    #[tokio::test]
    async fn test_start_succeeds_with_explicit_factory_type() {
        let temp = TempDir::new().unwrap();
        let ctx = CliContext::with_temp_dir(temp.path()).await.unwrap();
        let factory = ctx
            .agent_registry
            .list_factory_types()
            .await
            .into_iter()
            .next()
            .unwrap();

        let result = run(&ctx, "typed-agent-ok", None, Some(&factory), false).await;
        assert!(result.is_ok());
        assert!(ctx.agent_registry.contains("typed-agent-ok").await);
    }
}
