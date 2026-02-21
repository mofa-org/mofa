//! `mofa plugin uninstall` command implementation

use crate::commands::backend::CliBackend;
use colored::Colorize;

/// Execute the `mofa plugin uninstall` command
pub fn run(name: &str, _force: bool) -> anyhow::Result<()> {
    println!("{} Uninstalling plugin: {}", "→".green(), name.cyan());
    let backend = CliBackend::discover()?;
    let plugin = backend.uninstall_plugin(name)?;
    println!("{} Plugin '{}' uninstalled", "✓".green(), plugin.name);

    Ok(())
}
