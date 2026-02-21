//! `mofa plugin uninstall` command implementation

use crate::context::CliContext;
use colored::Colorize;
use dialoguer::Confirm;
use mofa_kernel::agent::plugins::PluginRegistry;

/// Execute the `mofa plugin uninstall` command
pub async fn run(ctx: &CliContext, name: &str, force: bool) -> anyhow::Result<()> {
    // Check if plugin exists
    if !ctx.plugin_registry.contains(name) {
        anyhow::bail!("Plugin '{}' not found in registry", name);
    }

    if !force {
        let confirmed = Confirm::new()
            .with_prompt(format!("Uninstall plugin '{}'?", name))
            .default(false)
            .interact()?;

        if !confirmed {
            println!("{} Cancelled", "→".yellow());
            return Ok(());
        }
    }

    println!("{} Uninstalling plugin: {}", "→".green(), name.cyan());

    let removed = ctx
        .plugin_registry
        .unregister(name)
        .map_err(|e| anyhow::anyhow!("Failed to unregister plugin: {}", e))?;

    if removed {
        if let Some(mut spec) = ctx
            .plugin_store
            .get(name)
            .map_err(|e| anyhow::anyhow!("Failed to load plugin spec '{}': {}", name, e))?
        {
            spec.enabled = false;
            ctx.plugin_store
                .save(name, &spec)
                .map_err(|e| anyhow::anyhow!("Failed to persist plugin '{}': {}", name, e))?;
        }

        println!("{} Plugin '{}' uninstalled", "✓".green(), name);
    } else {
        println!("{} Plugin '{}' was not in the registry", "!".yellow(), name);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::CliContext;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_uninstall_persists_disabled_plugin_spec() {
        let temp = TempDir::new().unwrap();
        let ctx = CliContext::with_temp_dir(temp.path()).await.unwrap();

        run(&ctx, "http-plugin", true).await.unwrap();

        let spec = ctx.plugin_store.get("http-plugin").unwrap().unwrap();
        assert!(!spec.enabled);

        drop(ctx);
        let ctx2 = CliContext::with_temp_dir(temp.path()).await.unwrap();
        assert!(!ctx2.plugin_registry.contains("http-plugin"));
    }
}
