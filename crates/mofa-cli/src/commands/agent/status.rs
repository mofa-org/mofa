//! `mofa agent status` command implementation

use crate::CliError;
use crate::context::CliContext;
use colored::Colorize;

/// Execute the `mofa agent status` command
pub async fn run(ctx: &CliContext, agent_id: Option<&str>) -> Result<(), CliError> {
    if let Some(id) = agent_id {
        // Show status for a specific agent
        println!("{} Agent status: {}", "→".green(), id.cyan());
        println!();

        match ctx.agent_registry.get_metadata(id).await {
            Some(metadata) => {
                println!("  ID:           {}", metadata.id.cyan());
                println!("  Name:         {}", metadata.name.white());
                println!(
                    "  State:        {}",
                    format!("{:?}", metadata.state).green()
                );
                if let Some(desc) = &metadata.description {
                    println!("  Description:  {}", desc.white());
                }
                if let Some(ver) = &metadata.version {
                    println!("  Version:      {}", ver.white());
                }
                let caps = &metadata.capabilities;
                if !caps.tags.is_empty() {
                    let tags: Vec<_> = caps.tags.iter().cloned().collect();
                    println!("  Tags:         {}", tags.join(", ").white());
                }
            }
            None => {
                let persisted = ctx.agent_store.get(id).map_err(|e| {
                    CliError::StateError(format!("Failed to load persisted agent '{}': {}", id, e))
                })?;

                if let Some(entry) = persisted {
                    println!("  ID:           {}", entry.id.cyan());
                    println!("  Name:         {}", entry.name.white());
                    println!(
                        "  State:        {}",
                        format!("{} (persisted)", entry.state).yellow()
                    );
                    if let Some(desc) = entry.description {
                        println!("  Description:  {}", desc.white());
                    }
                    println!("  Source:       persisted store (not active in runtime)");
                } else {
                    println!("  Agent '{}' not found in registry or persisted store", id);
                    println!();
                    println!(
                        "  Use {} to see available agents.",
                        "mofa agent list".cyan()
                    );
                }
            }
        }
    } else {
        // Show summary of all agents
        println!("{} Agent Status Summary", "→".green());
        println!();

        let stats = ctx.agent_registry.stats().await;

        if stats.total_agents == 0 {
            println!("  No agents currently registered.");
            return Ok(());
        }

        println!("  Total agents: {}", stats.total_agents);
        if !stats.by_state.is_empty() {
            println!("  By state:");
            for (state, count) in &stats.by_state {
                println!("    {}: {}", state, count);
            }
        }
        if stats.factory_count > 0 {
            println!("  Factories:    {}", stats.factory_count);
        }
    }

    Ok(())
}
