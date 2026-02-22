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
        println!("{} Plugin '{}' uninstalled", "✓".green(), name);
    } else {
        println!(
            "{} Plugin '{}' was not in the registry",
            "!".yellow(),
            name
        );
    }

    Ok(())
}
