//! `mofa tool list` command implementation

use crate::commands::backend::CliBackend;
use crate::output::Table;
use colored::Colorize;

/// Execute the `mofa tool list` command
pub fn run(available: bool, enabled: bool) -> anyhow::Result<()> {
    println!("{} Listing tools", "→".green());

    if available {
        println!("  Showing available tools");
    } else if enabled {
        println!("  Showing enabled tools");
    }

    println!();

    let backend = CliBackend::discover()?;
    let filtered = if available {
        backend.list_tools(false)?
    } else {
        backend.list_tools(enabled)?
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
