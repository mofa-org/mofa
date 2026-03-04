//! `mofa plugin info` command implementation

use crate::CliError;
use crate::context::CliContext;
use colored::Colorize;
use mofa_kernel::agent::plugins::PluginRegistry;

/// Execute the `mofa plugin info` command
pub async fn run(ctx: &CliContext, name: &str) -> Result<(), CliError> {
    println!("{} Plugin information: {}", "→".green(), name.cyan());
    println!();

    match ctx.plugin_registry.get(name) {
        Some(plugin) => {
            let metadata = plugin.metadata();
            println!("  Name:           {}", plugin.name().cyan());
            println!("  Description:    {}", plugin.description().white());
            println!("  Version:        {}", metadata.version.white());
            println!(
                "  Stages:         {}",
                metadata
                    .stages
                    .iter()
                    .map(|s| format!("{:?}", s))
                    .collect::<Vec<_>>()
                    .join(", ")
                    .white()
            );
            if !metadata.custom.is_empty() {
                println!("  Custom attrs:");
                for (key, value) in &metadata.custom {
                    println!("    {}: {}", key, value);
                }
            }
        }
        None => {
            println!("  Plugin '{}' not found in registry", name);
            println!();
            println!(
                "  Use {} to see available plugins.",
                "mofa plugin list".cyan()
            );
        }
    }

    println!();
    Ok(())
}
