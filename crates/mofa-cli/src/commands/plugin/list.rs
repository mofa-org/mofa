//! `mofa plugin list` command implementation

use crate::commands::backend::CliBackend;
use crate::output::Table;
use colored::Colorize;

/// Execute the `mofa plugin list` command
pub fn run(installed_only: bool, available: bool) -> anyhow::Result<()> {
    println!("{} Listing plugins", "→".green());

    if installed_only {
        println!("  Showing installed plugins");
    } else if available {
        println!("  Showing available plugins");
    }

    println!();

    let backend = CliBackend::discover()?;
    let filtered: Vec<_> = if available {
        backend.list_plugins(false)?
    } else {
        backend.list_plugins(installed_only)?
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
