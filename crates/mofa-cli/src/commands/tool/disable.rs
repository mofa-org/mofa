//! `mofa tool disable` command implementation

use crate::context::CliContext;
use colored::Colorize;
use mofa_kernel::agent::components::tool::ToolRegistry;

/// Execute the `mofa tool disable` command
pub async fn run(ctx: &mut CliContext, name: &str, force: bool) -> anyhow::Result<()> {
    let name = name.trim();
    if name.is_empty() {
        anyhow::bail!("Tool name cannot be empty");
    }

    // Check if it exists in the store
    let mut spec = match ctx.tool_store.get(name)? {
        Some(s) => s,
        None => {
            anyhow::bail!(
                "Tool '{}' is not registered. Only installed tools can be disabled.",
                name
            );
        }
    };

    if !spec.enabled {
        anyhow::bail!("Tool '{}' is already disabled", name);
    }

    if !force {
        use dialoguer::Confirm;
        let p = format!("Are you sure you want to disable the {} tool?", name.cyan());
        let confirmed = Confirm::new().with_prompt(p).default(false).interact()?;

        if !confirmed {
            println!("Operation cancelled");
            return Ok(());
        }
    }

    println!("{} Disabling tool: {}", "→".green(), name.cyan());

    // Unregister from memory mapping
    ctx.tool_registry.unregister(name);

    // Update the spec and persist it
    spec.enabled = false;
    if let Err(e) = ctx.tool_store.save(name, &spec) {
        anyhow::bail!(
            "Failed to persist tool '{}' state over to disabled: {}",
            name,
            e
        );
    }

    println!("{} Tool '{}' disabled successfully", "✓".green(), name);

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::CliContext;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_disable_deactivates_enabled_tool() {
        let temp = TempDir::new().unwrap();
        let mut ctx = CliContext::with_temp_dir(temp.path()).await.unwrap();

        // Echo is enabled by default
        assert!(ctx.tool_registry.contains("echo"));

        let result = run(&mut ctx, "echo", true).await;
        assert!(result.is_ok(), "disable should succeed: {:?}", result);

        assert!(!ctx.tool_registry.contains("echo"));
        let persisted = ctx.tool_store.get("echo").unwrap().unwrap();
        assert!(!persisted.enabled);
    }

    #[tokio::test]
    async fn test_disable_rejects_unknown_tool() {
        let temp = TempDir::new().unwrap();
        let mut ctx = CliContext::with_temp_dir(temp.path()).await.unwrap();

        let result = run(&mut ctx, "nonexistent-tool", true).await;
        assert!(result.is_err());
        assert!(
            result.unwrap_err().to_string().contains("not registered"),
            "error should mention 'not registered'"
        );
    }

    #[tokio::test]
    async fn test_disable_rejects_already_disabled() {
        let temp = TempDir::new().unwrap();
        let mut ctx = CliContext::with_temp_dir(temp.path()).await.unwrap();

        // Force disable once
        run(&mut ctx, "echo", true).await.unwrap();

        // Try disable again
        let result = run(&mut ctx, "echo", true).await;
        assert!(result.is_err());
        assert!(
            result.unwrap_err().to_string().contains("already disabled"),
            "error should mention 'already disabled'"
        );
    }
}
