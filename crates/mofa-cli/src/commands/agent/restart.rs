//! `mofa agent restart` command implementation

use crate::CliError;
use crate::context::CliContext;
use colored::Colorize;

/// Execute the `mofa agent restart` command
pub async fn run(
    ctx: &CliContext,
    agent_id: &str,
    config: Option<&std::path::Path>,
) -> Result<(), CliError> {
    println!("{} Restarting agent: {}", "→".green(), agent_id.cyan());

    // Stop the agent if it's running
    if ctx.agent_registry.contains(agent_id).await {
        super::stop::run(ctx, agent_id, false).await?;
    } else {
        println!("  Agent was not running");
    }

    // Start it again
    super::start::run(ctx, agent_id, config, None, false).await?;

    println!("{} Agent '{}' restarted", "✓".green(), agent_id);

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::agent::{list, start, stop};
    use crate::context::CliContext;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_restart_chain_start_stop_restart_list() {
        let temp = TempDir::new().unwrap();
        let ctx = CliContext::with_temp_dir(temp.path()).await.unwrap();

        start::run(&ctx, "chain-agent", None, None, false)
            .await
            .unwrap();
        stop::run(&ctx, "chain-agent", false).await.unwrap();
        run(&ctx, "chain-agent", None).await.unwrap();

        assert!(ctx.agent_registry.contains("chain-agent").await);
        let persisted = ctx.agent_store.get("chain-agent").unwrap().unwrap();
        assert_eq!(persisted.state, "Running");

        list::run(&ctx, false, false).await.unwrap();
        list::run(&ctx, true, false).await.unwrap();
    }
}
