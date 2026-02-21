//! `mofa agent status` command implementation

use crate::commands::backend::CliBackend;
use colored::Colorize;

/// Execute the `mofa agent status` command
pub fn run(agent_id: Option<&str>) -> anyhow::Result<()> {
    let backend = CliBackend::discover()?;

    if let Some(id) = agent_id {
        let agent = backend.get_agent(id)?;
        println!("{} Agent status: {}", "→".green(), id.cyan());
        println!();
        println!("  ID:     {}", agent.id);
        println!("  Status: {}", agent.status.green());
        if let Some(uptime) = agent.uptime {
            println!("  Uptime: {}", uptime.white());
        }
    } else {
        let agents = backend.list_agents(false)?;
        println!("{} Agent Status", "→".green());
        println!();
        if agents.is_empty() {
            println!("  No agents currently tracked.");
        } else {
            println!("  Total tracked agents: {}", agents.len());
            let running = agents
                .iter()
                .filter(|agent| agent.status.eq_ignore_ascii_case("running"))
                .count();
            println!("  Running: {}", running.to_string().green());
            println!(
                "  Stopped: {}",
                (agents.len() - running).to_string().yellow()
            );
        }
    }

    Ok(())
}
