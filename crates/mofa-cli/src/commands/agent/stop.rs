//! `mofa agent stop` command implementation

use crate::context::CliContext;
use colored::Colorize;

/// Execute the `mofa agent stop` command
pub async fn run(
    ctx: &CliContext,
    agent_id: &str,
    force_persisted_stop: bool,
) -> anyhow::Result<()> {
    println!("{} Stopping agent: {}", "→".green(), agent_id.cyan());

    let previous_entry = ctx
        .agent_store
        .get(agent_id)
        .map_err(|e| anyhow::anyhow!("Failed to load persisted agent '{}': {}", agent_id, e))?;

    // When commands run in separate CLI invocations, runtime registry state can be absent.
    // In that case, treat stop as a persisted-state transition if the agent exists on disk.
    if !ctx.agent_registry.contains(agent_id).await {
        if let Some(mut entry) = previous_entry.clone() {
            if !force_persisted_stop {
                anyhow::bail!(
                    "Agent '{}' is not active in runtime registry. Use --force-persisted-stop to mark persisted state as Stopped.",
                    agent_id
                );
            }

            entry.state = "Stopped".to_string();
            ctx.agent_store
                .save(agent_id, &entry)
                .map_err(|e| anyhow::anyhow!("Failed to update agent '{}': {}", agent_id, e))?;

            println!(
                "{} Agent '{}' was not running; updated persisted state to Stopped",
                "!".yellow(),
                agent_id
            );
            return Ok(());
        }

        anyhow::bail!(
            "Agent '{}' not found in registry or persisted store",
            agent_id
        );
    }

    // Attempt graceful shutdown via the agent instance
    if let Some(agent) = ctx.agent_registry.get(agent_id).await {
        let mut agent_guard = agent.write().await;
        if let Err(e) = agent_guard.shutdown().await {
            println!("  {} Graceful shutdown failed: {}", "!".yellow(), e);
        }
    }

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

    if !removed && persisted_updated
        && let Some(previous) = previous_entry
    {
        ctx.agent_store.save(agent_id, &previous).map_err(|e| {
            anyhow::anyhow!(
                "Agent '{}' remained registered and failed to restore persisted state: {}",
                agent_id,
                e
            )
        })?;
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
        run(&ctx, "stop-agent", false).await.unwrap();

        assert!(!ctx.agent_registry.contains("stop-agent").await);
        let persisted = ctx.agent_store.get("stop-agent").unwrap().unwrap();
        assert_eq!(persisted.state, "Stopped");
    }

    #[tokio::test]
    async fn test_stop_returns_error_for_missing_agent() {
        let temp = TempDir::new().unwrap();
        let ctx = CliContext::with_temp_dir(temp.path()).await.unwrap();

        let result = run(&ctx, "missing-agent", false).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_stop_errors_when_registry_missing_even_if_persisted_exists() {
        let temp = TempDir::new().unwrap();
        let first_ctx = CliContext::with_temp_dir(temp.path()).await.unwrap();
        start::run(&first_ctx, "persisted-agent", None, None, false)
            .await
            .unwrap();

        // Simulate a new CLI process: persisted entry remains, runtime registry is empty.
        let second_ctx = CliContext::with_temp_dir(temp.path()).await.unwrap();
        assert!(!second_ctx.agent_registry.contains("persisted-agent").await);

        let result = run(&second_ctx, "persisted-agent", false).await;
        assert!(result.is_err());

        let persisted = second_ctx
            .agent_store
            .get("persisted-agent")
            .unwrap()
            .unwrap();
        assert_eq!(persisted.state, "Running");
    }

    #[tokio::test]
    async fn test_stop_force_persisted_stop_updates_state_when_registry_missing() {
        let temp = TempDir::new().unwrap();
        let first_ctx = CliContext::with_temp_dir(temp.path()).await.unwrap();
        start::run(&first_ctx, "persisted-agent-force", None, None, false)
            .await
            .unwrap();

        let second_ctx = CliContext::with_temp_dir(temp.path()).await.unwrap();
        assert!(
            !second_ctx
                .agent_registry
                .contains("persisted-agent-force")
                .await
        );

        run(&second_ctx, "persisted-agent-force", true)
            .await
            .unwrap();

        let persisted = second_ctx
            .agent_store
            .get("persisted-agent-force")
            .unwrap()
            .unwrap();
        assert_eq!(persisted.state, "Stopped");
    }
}
