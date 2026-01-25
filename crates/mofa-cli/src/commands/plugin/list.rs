//! `mofa plugin list` command implementation

use crate::output::Table;
use colored::Colorize;
use serde::Serialize;

/// Execute the `mofa plugin list` command
pub fn run(installed_only: bool, available: bool) -> anyhow::Result<()> {
    println!("{} Listing plugins", "→".green());

    if installed_only {
        println!("  Showing installed plugins");
    } else if available {
        println!("  Showing available plugins");
    }

    println!();

    // TODO: Implement actual plugin discovery from plugin registry

    let plugins = vec![
        PluginInfo {
            name: "http-server".to_string(),
            version: "0.1.0".to_string(),
            description: "HTTP server plugin for exposing agents via REST API".to_string(),
            installed: true,
        },
        PluginInfo {
            name: "postgres-persistence".to_string(),
            version: "0.1.0".to_string(),
            description: "PostgreSQL persistence plugin for session storage".to_string(),
            installed: true,
        },
        PluginInfo {
            name: "web-scraper".to_string(),
            version: "0.2.0".to_string(),
            description: "Web scraping tool for content extraction".to_string(),
            installed: false,
        },
        PluginInfo {
            name: "code-interpreter".to_string(),
            version: "0.1.0".to_string(),
            description: "Sandboxed code execution environment".to_string(),
            installed: false,
        },
    ];

    let filtered: Vec<_> = if installed_only {
        plugins.iter().filter(|p| p.installed).cloned().collect()
    } else {
        plugins
    };

    if filtered.is_empty() {
        println!("  No plugins found.");
        return Ok(());
    }

    let json = serde_json::to_value(&filtered)?;
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
    installed: bool,
}
