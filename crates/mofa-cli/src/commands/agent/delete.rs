//! `mofa agent delete` command implementation

use crate::context::CliContext;
use colored::Colorize;

/// Execute the `mofa agent delete` command.
///
/// Deletes persisted CLI agent metadata and unregisters any live runtime agent.
/// If the agent is currently active in the runtime registry, `force` is required.
pub async fn run(ctx: &CliContext, agent_id: &str, force: bool) -> anyhow::Result<()> {
    println!("{} Deleting agent: {}", "→".green(), agent_id.cyan());

    let in_registry = ctx.agent_registry.contains(agent_id).await;
    let in_store = ctx
        .agent_store
        .get(agent_id)
        .map_err(|e| anyhow::anyhow!("Failed to query persisted agent '{}': {}", agent_id, e))?
        .is_some();
    let in_persistent_registry = ctx.persistent_agents.exists(agent_id).await;

    if !in_registry && !in_store && !in_persistent_registry {
        anyhow::bail!("Agent '{}' not found", agent_id);
    }

    if in_registry && !force {
        anyhow::bail!(
            "Agent '{}' is currently active. Stop it first or pass --force to delete anyway.",
            agent_id
        );
    }

    if in_registry {
        if let Some(agent) = ctx.agent_registry.get(agent_id).await {
            let mut guard = agent.write().await;
            if let Err(e) = guard.shutdown().await {
                println!("  {} Graceful shutdown failed: {}", "!".yellow(), e);
            }
        }

        let removed_live = ctx.agent_registry.unregister(agent_id).await.map_err(|e| {
            anyhow::anyhow!("Failed to unregister live agent '{}': {}", agent_id, e)
        })?;
        if removed_live {
            println!("  {} Removed live runtime registration", "•".bright_black());
        }
    }

    let removed_store = ctx
        .agent_store
        .delete(agent_id)
        .map_err(|e| anyhow::anyhow!("Failed to delete persisted agent '{}': {}", agent_id, e))?;
    if removed_store {
        println!("  {} Removed persisted CLI metadata", "•".bright_black());
    }

    let removed_persistent_registry =
        ctx.persistent_agents.remove(agent_id).await.map_err(|e| {
            anyhow::anyhow!(
                "Failed to delete persistent registry agent '{}': {}",
                agent_id,
                e
            )
        })?;
    if removed_persistent_registry {
        println!(
            "  {} Removed persistent runtime metadata",
            "•".bright_black()
        );
    }
    ctx.persistent_agents.untrack_process(agent_id).await;

    println!("{} Agent '{}' deleted", "✓".green(), agent_id);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::agent::start;
    use crate::context::CliContext;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_delete_missing_agent_errors() {
        let temp = TempDir::new().unwrap();
        let ctx = CliContext::with_temp_dir(temp.path()).await.unwrap();

        let result = run(&ctx, "missing-agent", false).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_delete_persisted_agent_without_runtime_registry_entry() {
        let temp = TempDir::new().unwrap();
        let first_ctx = CliContext::with_temp_dir(temp.path()).await.unwrap();
        start::run(&first_ctx, "persisted-delete", None, None, false)
            .await
            .unwrap();

        // Simulate a fresh process where runtime registry is empty but persisted data exists.
        let second_ctx = CliContext::with_temp_dir(temp.path()).await.unwrap();
        assert!(!second_ctx.agent_registry.contains("persisted-delete").await);

        run(&second_ctx, "persisted-delete", false).await.unwrap();
        let persisted = second_ctx.agent_store.get("persisted-delete").unwrap();
        assert!(persisted.is_none());
    }

    #[tokio::test]
    async fn test_delete_running_agent_requires_force() {
        let temp = TempDir::new().unwrap();
        let ctx = CliContext::with_temp_dir(temp.path()).await.unwrap();
        start::run(&ctx, "running-delete", None, None, false)
            .await
            .unwrap();
        assert!(ctx.agent_registry.contains("running-delete").await);

        let result = run(&ctx, "running-delete", false).await;
        assert!(result.is_err());
        assert!(ctx.agent_registry.contains("running-delete").await);
    }

    #[tokio::test]
    async fn test_delete_running_agent_with_force_removes_everything() {
        let temp = TempDir::new().unwrap();
        let ctx = CliContext::with_temp_dir(temp.path()).await.unwrap();
        start::run(&ctx, "running-force-delete", None, None, false)
            .await
            .unwrap();
        assert!(ctx.agent_registry.contains("running-force-delete").await);

        run(&ctx, "running-force-delete", true).await.unwrap();

        assert!(!ctx.agent_registry.contains("running-force-delete").await);
        assert!(
            ctx.agent_store
                .get("running-force-delete")
                .unwrap()
                .is_none()
        );
    }
}
