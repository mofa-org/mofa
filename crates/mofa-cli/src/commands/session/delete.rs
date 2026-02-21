//! `mofa session delete` command implementation

use crate::commands::backend::CliBackend;
use colored::Colorize;

/// Execute the `mofa session delete` command
pub fn run(session_id: &str, force: bool) -> anyhow::Result<()> {
    if !force {
        println!("{} Delete session: {}?", "→".yellow(), session_id.cyan());
        println!("  This action cannot be undone.");
        println!();
        println!("  Use --force to skip confirmation.");
        // TODO: Add actual confirmation prompt
        return Ok(());
    }

    println!("{} Deleting session: {}", "→".green(), session_id.cyan());
    let backend = CliBackend::discover()?;
    backend.delete_session(session_id)?;

    println!("{} Session '{}' deleted", "✓".green(), session_id);

    Ok(())
}
