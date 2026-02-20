//! `mofa agent status` command implementation

use colored::Colorize;

/// Execute the `mofa agent status` command
pub fn run(agent_id: Option<&str>) -> anyhow::Result<()> {
    if let Some(id) = agent_id {
        // Show status for a specific agent
        println!("{} Agent status: {}", "→".green(), id.cyan());
        println!();
        println!("  ID:     {}", id);
        println!("  Status: {}", "Running".green());
        println!("  Uptime: {}", "5m 32s".white());
    } else {
        // Show summary of all agents
        println!("{} Agent Status", "→".green());
        println!();
        println!("  No agents currently running.");
    }

    Ok(())
}
