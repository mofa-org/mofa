//! `mofa session list` command implementation

use crate::commands::backend::CliBackend;
use crate::output::Table;
use colored::Colorize;

/// Execute the `mofa session list` command
pub fn run(agent_id: Option<&str>, limit: Option<usize>) -> anyhow::Result<()> {
    println!("{} Listing sessions", "→".green());

    if let Some(agent) = agent_id {
        println!("  Filtering by agent: {}", agent.cyan());
    }

    if let Some(n) = limit {
        println!("  Limit: {}", n);
    }

    println!();

    let backend = CliBackend::discover()?;
    let limited = backend.list_sessions(agent_id, limit)?;

    if limited.is_empty() {
        println!("  No sessions found.");
        return Ok(());
    }

    let json = serde_json::to_value(&limited)?;
    if let Some(arr) = json.as_array() {
        let table = Table::from_json_array(arr);
        println!("{}", table);
    }

    Ok(())
}
