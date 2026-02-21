//! `mofa tool info` command implementation

use crate::commands::backend::CliBackend;
use colored::Colorize;

/// Execute the `mofa tool info` command
pub fn run(name: &str) -> anyhow::Result<()> {
    println!("{} Tool information: {}", "→".green(), name.cyan());
    println!();

    let backend = CliBackend::discover()?;
    let tool = backend.get_tool(name)?;

    println!("  Name:           {}", tool.name.cyan());
    println!("  Description:    {}", tool.description.white());
    println!("  Version:        {}", tool.version.white());
    println!(
        "  Enabled:        {}",
        if tool.enabled {
            "Yes".green()
        } else {
            "No".yellow()
        }
    );
    println!();

    Ok(())
}
