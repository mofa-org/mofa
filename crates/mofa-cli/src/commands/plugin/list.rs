//! `mofa plugin list` command implementation

use crate::context::CliContext;
use crate::output::Table;
use colored::Colorize;
use mofa_kernel::agent::plugins::PluginRegistry;
use serde::Serialize;

/// Execute the `mofa plugin list` command
pub async fn run(ctx: &CliContext, _installed_only: bool, _available: bool) -> anyhow::Result<()> {
    println!("{} Listing plugins", "→".green());
    println!();

    let plugins = ctx.plugin_registry.list();

    if plugins.is_empty() {
        println!("  No plugins registered.");
        println!();
        println!("  Plugins can be registered programmatically via the SDK.");
        return Ok(());
    }

    let infos: Vec<PluginInfo> = plugins
        .iter()
        .map(|p| {
            let metadata = p.metadata();
            PluginInfo {
                name: p.name().to_string(),
                version: metadata.version.clone(),
                description: p.description().to_string(),
                stages: metadata
                    .stages
                    .iter()
                    .map(|s| format!("{:?}", s))
                    .collect::<Vec<_>>()
                    .join(", "),
            }
        })
        .collect();

    let json = serde_json::to_value(&infos)?;
    if let Some(arr) = json.as_array() {
        let table = Table::from_json_array(arr);
        println!("{}", table);
    }

    Ok(())
}

#[derive(Debug, Clone, Serialize)]
struct PluginInfo {
    name: String,
    version: String,
    description: String,
    stages: String,
}
