//! `mofa tool list` command implementation

use crate::CliError;
use crate::context::CliContext;
use crate::output::Table;
use colored::Colorize;
use mofa_kernel::agent::components::tool::ToolRegistry;
use serde::Serialize;

/// Execute the `mofa tool list` command
pub async fn run(ctx: &CliContext, _available: bool, _enabled: bool) -> Result<(), CliError> {
    println!("{} Listing tools", "→".green());
    println!();

    let descriptors = ctx.tool_registry.list();

    if descriptors.is_empty() {
        println!("  No tools registered.");
        println!();
        println!("  Tools can be registered programmatically via the SDK.");
        return Ok(());
    }

    let tools: Vec<ToolInfo> = descriptors
        .iter()
        .map(|d| {
            let source = ctx
                .tool_registry
                .get_source(&d.name)
                .map(|s| format!("{:?}", s))
                .unwrap_or_else(|| "unknown".to_string());
            ToolInfo {
                name: d.name.clone(),
                description: d.description.clone(),
                category: d.metadata.category.clone().unwrap_or_default(),
                source,
            }
        })
        .collect();

    let json = serde_json::to_value(&tools)?;
    if let Some(arr) = json.as_array() {
        let table = Table::from_json_array(arr);
        println!("{}", table);
    }

    Ok(())
}

#[derive(Debug, Clone, Serialize)]
struct ToolInfo {
    name: String,
    description: String,
    category: String,
    source: String,
}
