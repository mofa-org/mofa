//! `mofa agent stop` command implementation

use crate::CliError;
use crate::context::CliContext;
use colored::Colorize;
use tracing::info;

/// Execute the `mofa agent stop` command
pub async fn run(
    ctx: &CliContext,
    agent_id: &str,
    force_persisted_stop: bool,
) -> Result<(), CliError> {
    println!("{} Stopping agent: {}", "→".green(), agent_id.cyan());

    // Check if agent exists in registry or store
    let in_registry = ctx.agent_registry.contains(agent_id).await;

    let previous_entry = ctx
        .agent_store
        .get(agent_id)
        .map_err(|e| CliError::StateError(format!("Failed to load persisted agent '{}': {}", agent_id, e)))?;

    let in_store = previous_entry.is_some();

    if !in_registry && !in_store {
        return Err(CliError::StateError(format!("Agent '{}' not found", agent_id)));
    }

    // When commands run in separate CLI invocations, runtime registry state can be absent.
    // In that case, if force_persisted_stop is true, update the persisted state.
    if !in_registry
        && in_store
        && force_persisted_stop
        && let Some(mut entry) = previous_entry
    {
        entry.state = "Stopped".to_string();
        ctx.agent_store
            .save(agent_id, &entry)
            .map_err(|e| CliError::StateError(format!("Failed to update agent '{}': {}", agent_id, e)))?;

        println!(
            "{} Agent '{}' persisted state updated to Stopped",
            "✓".green(),
            agent_id
        );
        return Ok(());
    }

    // If not in registry and no force flag, error out
    if !in_registry {
        return Err(CliError::StateError(format!(
            "Agent '{}' is not active in runtime registry. Use --force-persisted-stop to update persisted state.",
            agent_id
        )));
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
            .map_err(|e| CliError::StateError(format!("Failed to update agent '{}': {}", agent_id, e)))?;
        true
    } else {
        false
    };

    // Unregister from the registry after persistence update so failures do not leave stale state.
    let removed = ctx
        .agent_registry
        .unregister(agent_id)
        .await
        .map_err(|e| CliError::StateError(format!("Failed to unregister agent: {}", e)))?;

    if !removed
        && persisted_updated
        && let Some(previous) = previous_entry
    {
        ctx.agent_store.save(agent_id, &previous).map_err(|e| {
            CliError::StateError(format!(
                "Agent '{}' remained registered and failed to restore persisted state: {}",
                agent_id,
                e
            ))
        })?;
    }

    if removed {
        println!(
            "{} Agent '{}' stopped and unregistered",
            "✓".green(),
            agent_id
        );
    } else {
        println!("  {} Agent is not running", "!".yellow());
    }

    println!("{} Agent '{}' stopped", "✓".green(), agent_id);

    info!("Agent '{}' stopped", agent_id);

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::agent::start;
    use crate::context::CliContext;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_stop_updates_state_and_unregisters_agent() -> Result<(), Box<dyn std::error::Error>> {
        let temp = TempDir::new().expect("failed to create temporary directory");
        let ctx = CliContext::with_temp_dir(temp.path()).await?;

        start::run(&ctx, "stop-agent", None, None, false).await?;
        run(&ctx, "stop-agent", false).await?;

        assert!(!ctx.agent_registry.contains("stop-agent").await);
        let persisted = ctx.agent_store.get("stop-agent")?.unwrap();
        assert_eq!(persisted.state, "Stopped");
        Ok(())
    }

    #[tokio::test]
    async fn test_stop_returns_error_for_missing_agent() -> Result<(), Box<dyn std::error::Error>> {
        let temp = TempDir::new().expect("failed to create temporary directory");
        let ctx = CliContext::with_temp_dir(temp.path()).await?;

        let result = run(&ctx, "missing-agent", false).await;
        assert!(result.is_err());
        Ok(())
    }

    #[tokio::test]
    async fn test_stop_errors_when_registry_missing_even_if_persisted_exists() -> Result<(), Box<dyn std::error::Error>>
    {
        let temp = TempDir::new().expect("failed to create temporary directory");
        let first_ctx = CliContext::with_temp_dir(temp.path())
            .await
            .expect("Failed to initialize test CLI context");
        start::run(&first_ctx, "persisted-agent", None, None, false).await?;

        // Simulate a new CLI process: persisted entry remains, runtime registry is empty.
        let second_ctx = CliContext::with_temp_dir(temp.path())
            .await
            .expect("Failed to initialize test CLI context");
        assert!(!second_ctx.agent_registry.contains("persisted-agent").await);

        let result = run(&second_ctx, "persisted-agent", false).await;
        assert!(result.is_err());

        let persisted = second_ctx
            .agent_store
            .get("persisted-agent")
            .expect("Request to agent_store failed")
            .expect("Agent 'persisted-agent' should exist in store");
        assert_eq!(persisted.state, "Running");
        Ok(())
    }

    #[tokio::test]
    async fn test_stop_force_persisted_stop_updates_state_when_registry_missing()
    -> Result<(), Box<dyn std::error::Error>> {
        let temp = TempDir::new().expect("failed to create temporary directory");
        let first_ctx = CliContext::with_temp_dir(temp.path())
            .await
            .expect("Failed to initialize test CLI context");
        start::run(&first_ctx, "persisted-agent-force", None, None, false).await?;

        let second_ctx = CliContext::with_temp_dir(temp.path())
            .await
            .expect("Failed to initialize test CLI context");
        assert!(
            !second_ctx
                .agent_registry
                .contains("persisted-agent-force")
                .await
        );

        run(&second_ctx, "persisted-agent-force", true).await?;

        let persisted = second_ctx
            .agent_store
            .get("persisted-agent-force")?
            .unwrap();
        assert_eq!(persisted.state, "Stopped");
        Ok(())
    }
}
