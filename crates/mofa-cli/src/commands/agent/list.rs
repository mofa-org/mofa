//! `mofa agent list` command implementation

use crate::commands::backend::CliBackend;
use crate::output::Table;
use colored::Colorize;

/// Execute the `mofa agent list` command
pub fn run(running_only: bool, show_all: bool) -> anyhow::Result<()> {
    println!("{} Listing agents", "→".green());

    if running_only {
        println!("  Showing running agents only");
    } else if show_all {
        println!("  Showing all agents");
    }

    println!();

    let backend = CliBackend::discover()?;
    let filtered = backend.list_agents(running_only)?;

    if filtered.is_empty() {
        println!("  No agents found.");
        return Ok(());
    }

    // Display as table
    let json = serde_json::to_value(&filtered)?;
    if let Some(arr) = json.as_array() {
        let table = Table::from_json_array(arr);
        println!("{}", table);
    }

    Ok(())
}
