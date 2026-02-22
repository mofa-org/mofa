//! `mofa agent stop` command implementation

use crate::context::CliContext;
use colored::Colorize;

/// Execute the `mofa agent stop` command
pub async fn run(ctx: &CliContext, agent_id: &str) -> anyhow::Result<()> {
    println!("{} Stopping agent: {}", "→".green(), agent_id.cyan());

    // Check if agent exists
    if !ctx.agent_registry.contains(agent_id).await {
        anyhow::bail!("Agent '{}' not found in registry", agent_id);
    }

    // Attempt graceful shutdown via the agent instance
    if let Some(agent) = ctx.agent_registry.get(agent_id).await {
        let mut agent_guard = agent.write().await;
        if let Err(e) = agent_guard.shutdown().await {
            println!(
                "  {} Graceful shutdown failed: {}",
                "!".yellow(),
                e
            );
        }
    }

    // Unregister from the registry
    let removed = ctx
        .agent_registry
        .unregister(agent_id)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to unregister agent: {}", e))?;

    if removed {
        println!("{} Agent '{}' stopped and unregistered", "✓".green(), agent_id);
    } else {
        println!(
            "{} Agent '{}' was not in the registry",
            "!".yellow(),
            agent_id
        );
    }

    Ok(())
}
