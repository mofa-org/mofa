//! `mofa plugin info` command implementation

use crate::commands::backend::CliBackend;
use colored::Colorize;

/// Execute the `mofa plugin info` command
pub fn run(name: &str) -> anyhow::Result<()> {
    println!("{} Plugin information: {}", "→".green(), name.cyan());
    println!();

    let backend = CliBackend::discover()?;
    let plugin = backend.get_plugin(name)?;

    println!("  Name:           {}", plugin.name.cyan());
    println!("  Version:        {}", plugin.version.white());
    println!("  Description:    {}", plugin.description.white());
    println!("  Author:         {}", plugin.author.white());
    if let Some(repo) = plugin.repository {
        println!("  Repository:     {}", repo.blue());
    }
    if let Some(license) = plugin.license {
        println!("  License:        {}", license.white());
    }
    println!(
        "  Installed:      {}",
        if plugin.installed {
            "Yes".green()
        } else {
            "No".yellow()
        }
    );
    println!();

    Ok(())
}
