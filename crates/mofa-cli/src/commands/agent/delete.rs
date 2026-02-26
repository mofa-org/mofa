//! `mofa agent delete` command implementation

use crate::context::CliContext;
use colored::Colorize;
use dialoguer::Confirm;

/// Execute the `mofa agent delete` command.
///
/// The primary authoritative store is `agent_store` (the same one `agent start`
/// writes to).  `persistent_agents` is cleaned up as a secondary step so that
/// both stores stay in sync regardless of which one happens to have the entry.
pub async fn run(ctx: &CliContext, agent_id: &str, force: bool) -> anyhow::Result<()> {
    println!("{} Deleting agent: {}", "→".green(), agent_id.cyan());

    // 1. Check existence — the agent must be known to at least one store.
    let in_agent_store = ctx
        .agent_store
        .get(agent_id)
        .unwrap_or(None)
        .is_some();
    let in_persistent = ctx.persistent_agents.exists(agent_id).await;

    if !in_agent_store && !in_persistent {
        anyhow::bail!("Agent '{}' not found", agent_id);
    }

    // 2. Refuse to delete a running agent.
    if ctx.agent_registry.contains(agent_id).await {
        anyhow::bail!(
            "Agent '{}' is currently running. Stop it first: mofa agent stop {}",
            agent_id,
            agent_id
        );
    }

    // 3. Interactive confirmation (skipped with --force).
    if !force {
        let confirmed = Confirm::new()
            .with_prompt(format!(
                "Are you sure you want to completely delete agent '{}'?",
                agent_id
            ))
            .default(false)
            .interact()?;

        if !confirmed {
            println!("{} Deletion cancelled.", "→".yellow());
            return Ok(());
        }
    }

    // 4. Delete from both stores.  We intentionally tolerate "not found" from
    //    either store — the important thing is that the agent is gone from both.
    let removed_from_store = ctx.agent_store.delete(agent_id)?;
    let removed_from_persistent = ctx.persistent_agents.remove(agent_id).await?;

    if !removed_from_store && !removed_from_persistent {
        // Both stores said "nothing to remove" — shouldn't happen since we
        // checked existence above, but guard against races.
        anyhow::bail!("Agent '{}' not found during deletion", agent_id);
    }

    println!(
        "{} Successfully deleted agent '{}'",
        "✓".green(),
        agent_id.cyan()
    );

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::agent::{start, stop};
    use crate::context::CliContext;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_delete_missing_agent_returns_error() {
        let temp = TempDir::new().unwrap();
        let ctx = CliContext::with_temp_dir(temp.path()).await.unwrap();

        let result = run(&ctx, "missing-agent", true).await;
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().to_string(),
            "Agent 'missing-agent' not found"
        );
    }

    #[tokio::test]
    async fn test_delete_running_agent_returns_error() {
        let temp = TempDir::new().unwrap();
        let ctx = CliContext::with_temp_dir(temp.path()).await.unwrap();

        start::run(&ctx, "running-agent", None, None, false)
            .await
            .unwrap();

        let result = run(&ctx, "running-agent", true).await;
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("is currently running")
        );

        // Cleanup
        stop::run(&ctx, "running-agent", false).await.unwrap();
    }

    #[tokio::test]
    async fn test_delete_stopped_agent_succeeds() {
        let temp = TempDir::new().unwrap();
        let ctx = CliContext::with_temp_dir(temp.path()).await.unwrap();

        start::run(&ctx, "to-be-deleted", None, None, false)
            .await
            .unwrap();
        stop::run(&ctx, "to-be-deleted", false).await.unwrap();

        // Verify it exists before deletion
        assert!(ctx.agent_store.get("to-be-deleted").unwrap().is_some());

        let result = run(&ctx, "to-be-deleted", true).await;
        assert!(result.is_ok(), "delete failed: {:?}", result);

        // Both stores should be empty
        assert!(ctx.agent_store.get("to-be-deleted").unwrap().is_none());
        assert!(!ctx.persistent_agents.exists("to-be-deleted").await);
    }

    #[tokio::test]
    async fn test_delete_is_idempotent_across_context_restarts() {
        let temp = TempDir::new().unwrap();

        // First context: start and stop an agent
        {
            let ctx = CliContext::with_temp_dir(temp.path()).await.unwrap();
            start::run(&ctx, "ephemeral", None, None, false)
                .await
                .unwrap();
            stop::run(&ctx, "ephemeral", false).await.unwrap();
        }

        // Second context (simulating a new CLI invocation): delete
        {
            let ctx = CliContext::with_temp_dir(temp.path()).await.unwrap();
            assert!(
                ctx.agent_store.get("ephemeral").unwrap().is_some(),
                "agent should survive context restart"
            );

            let result = run(&ctx, "ephemeral", true).await;
            assert!(result.is_ok(), "delete failed: {:?}", result);
            assert!(ctx.agent_store.get("ephemeral").unwrap().is_none());
        }

        // Third context: verify it stays gone
        {
            let ctx = CliContext::with_temp_dir(temp.path()).await.unwrap();
            assert!(ctx.agent_store.get("ephemeral").unwrap().is_none());
            assert!(!ctx.persistent_agents.exists("ephemeral").await);
        }
    }
}
