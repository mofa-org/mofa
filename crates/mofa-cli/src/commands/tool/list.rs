//! `mofa tool list` command implementation

use crate::output::Table;
use colored::Colorize;
use serde::Serialize;

/// Execute the `mofa tool list` command
pub fn run(available: bool, enabled: bool) -> anyhow::Result<()> {
    println!("{} Listing tools", "→".green());

    if available {
        println!("  Showing available tools");
    } else if enabled {
        println!("  Showing enabled tools");
    }

    println!();

    // TODO: Implement actual tool discovery from tool registry

    let tools = vec![
        ToolInfo {
            name: "web-search".to_string(),
            description: "Search the web for information".to_string(),
            enabled: true,
        },
        ToolInfo {
            name: "calculator".to_string(),
            description: "Perform mathematical calculations".to_string(),
            enabled: true,
        },
        ToolInfo {
            name: "code-executor".to_string(),
            description: "Execute code in a sandboxed environment".to_string(),
            enabled: false,
        },
        ToolInfo {
            name: "file-operations".to_string(),
            description: "Read, write, and manipulate files".to_string(),
            enabled: false,
        },
    ];

    let filtered: Vec<_> = if enabled {
        tools.iter().filter(|t| t.enabled).cloned().collect()
    } else {
        tools
    };

    if filtered.is_empty() {
        println!("  No tools found.");
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
struct ToolInfo {
    name: String,
    description: String,
    enabled: bool,
}
