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
            println!("  {} Graceful shutdown failed: {}", "!".yellow(), e);
        }
    }

    let previous_entry = ctx
        .agent_store
        .get(agent_id)
        .map_err(|e| anyhow::anyhow!("Failed to load persisted agent '{}': {}", agent_id, e))?;

    let persisted_updated = if let Some(mut entry) = previous_entry.clone() {
        entry.state = "Stopped".to_string();
        ctx.agent_store
            .save(agent_id, &entry)
            .map_err(|e| anyhow::anyhow!("Failed to update agent '{}': {}", agent_id, e))?;
        true
    } else {
        false
    };

    // Unregister from the registry after persistence update so failures do not leave stale state.
    let removed = ctx
        .agent_registry
        .unregister(agent_id)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to unregister agent: {}", e))?;

    if !removed && persisted_updated {
        if let Some(previous) = previous_entry {
            ctx.agent_store.save(agent_id, &previous).map_err(|e| {
                anyhow::anyhow!(
                    "Agent '{}' remained registered and failed to restore persisted state: {}",
                    agent_id,
                    e
                )
            })?;
        }
    }

    if removed {
        println!(
            "{} Agent '{}' stopped and unregistered",
            "✓".green(),
            agent_id
        );
    } else {
        println!(
            "{} Agent '{}' was not in the registry",
            "!".yellow(),
            agent_id
        );
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::agent::start;
    use crate::context::CliContext;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_stop_updates_state_and_unregisters_agent() {
        let temp = TempDir::new().unwrap();
        let ctx = CliContext::with_temp_dir(temp.path()).await.unwrap();

        start::run(&ctx, "stop-agent", None, None, false)
            .await
            .unwrap();
        run(&ctx, "stop-agent").await.unwrap();

        assert!(!ctx.agent_registry.contains("stop-agent").await);
        let persisted = ctx.agent_store.get("stop-agent").unwrap().unwrap();
        assert_eq!(persisted.state, "Stopped");
    }

    #[tokio::test]
    async fn test_stop_returns_error_for_missing_agent() {
        let temp = TempDir::new().unwrap();
        let ctx = CliContext::with_temp_dir(temp.path()).await.unwrap();

        let result = run(&ctx, "missing-agent").await;
        assert!(result.is_err());
    }
}
