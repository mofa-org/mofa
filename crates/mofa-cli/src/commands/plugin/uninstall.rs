//! `mofa plugin uninstall` command implementation

use crate::CliError;
use crate::context::CliContext;
use colored::Colorize;
use dialoguer::Confirm;
use mofa_kernel::agent::plugins::PluginRegistry;

/// Execute the `mofa plugin uninstall` command
pub async fn run(ctx: &CliContext, name: &str, force: bool) -> Result<(), CliError> {
    // Check if plugin exists
    if !ctx.plugin_registry.contains(name) {
        return Err(CliError::PluginError(format!("Plugin '{}' not found in registry", name)));
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

    let previous_spec = ctx
        .plugin_store
        .get(name)
        .map_err(|e| CliError::PluginError(format!("Failed to load plugin spec '{}': {}", name, e)))?;

    let persisted_updated = if let Some(mut spec) = previous_spec.clone() {
        spec.enabled = false;
        ctx.plugin_store
            .save(name, &spec)
            .map_err(|e| CliError::PluginError(format!("Failed to persist plugin '{}': {}", name, e)))?;
        true
    } else {
        false
    };

    let removed = ctx
        .plugin_registry
        .unregister(name)
        .map_err(|e| CliError::PluginError(format!("Failed to unregister plugin: {}", e)))?;

    if !removed
        && persisted_updated
        && let Some(previous) = previous_spec
    {
        ctx.plugin_store.save(name, &previous).map_err(|e| {
            CliError::PluginError(format!(
                "Plugin '{}' remained registered and failed to restore persisted state: {}",
                name,
                e
            ))
        })?;
    }

    if removed {
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
