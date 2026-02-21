//! `mofa agent restart` command implementation

use crate::commands::backend::CliBackend;
use colored::Colorize;

/// Execute the `mofa agent restart` command
pub fn run(agent_id: &str, config: Option<&std::path::Path>) -> anyhow::Result<()> {
    println!("{} Restarting agent: {}", "→".green(), agent_id.cyan());
    let backend = CliBackend::discover()?;
    let restarted = backend.restart_agent(agent_id, config)?;
    println!("{} Agent '{}' restarted", "✓".green(), restarted.id);

    Ok(())
}
