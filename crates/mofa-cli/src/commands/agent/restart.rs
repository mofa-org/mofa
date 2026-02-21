//! `mofa agent restart` command implementation

use crate::context::CliContext;
use colored::Colorize;

/// Execute the `mofa agent restart` command
pub async fn run(
    ctx: &CliContext,
    agent_id: &str,
    config: Option<&std::path::Path>,
) -> anyhow::Result<()> {
    println!("{} Restarting agent: {}", "→".green(), agent_id.cyan());

    // Stop the agent if it's running
    if ctx.agent_registry.contains(agent_id).await {
        // Attempt graceful shutdown
        if let Some(agent) = ctx.agent_registry.get(agent_id).await {
            let mut agent_guard = agent.write().await;
            if let Err(e) = agent_guard.shutdown().await {
                println!("  {} Graceful shutdown failed: {}", "!".yellow(), e);
            }
        }

        ctx.agent_registry
            .unregister(agent_id)
            .await
            .map_err(|e| anyhow::anyhow!("Failed to unregister agent: {}", e))?;

        println!("  Agent stopped");
    } else {
        println!("  Agent was not running");
    }

    // Start it again
    super::start::run(ctx, agent_id, config, false).await?;

    println!("{} Agent '{}' restarted", "✓".green(), agent_id);

    Ok(())
}
