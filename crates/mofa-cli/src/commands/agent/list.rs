//! `mofa agent list` command implementation

use crate::context::CliContext;
use crate::output::Table;
use colored::Colorize;
use serde::Serialize;

/// Execute the `mofa agent list` command
pub async fn run(ctx: &CliContext, running_only: bool, _show_all: bool) -> anyhow::Result<()> {
    println!("{} Listing agents", "→".green());
    println!();

    // Load all agents from persistent storage
    let all_agents = ctx.persistent_agents.list().await;

    if all_agents.is_empty() {
        println!("  No agents registered.");
        println!();
        println!(
            "  Use {} to register an agent.",
            "mofa agent start <agent_id>".cyan()
        );
        return Ok(());
    }

    // Filter agents based on flags
    let agents: Vec<AgentInfo> = all_agents
        .iter()
        .filter(|a| {
            if running_only {
                a.last_state == crate::state::AgentProcessState::Running
            } else {
                true
            }
        })
        .map(|m| {
            let status = m.last_state.to_string();
            let process_id = m.process_id.map(|pid| pid.to_string());

            AgentInfo {
                id: m.id.clone(),
                name: m.name.clone(),
                status,
                process_id,
                starts: m.start_count,
                last_started: m.last_started.map(format_timestamp),
                description: m.description.clone(),
            }
        })
        .collect();

    if agents.is_empty() {
        if running_only {
            println!("  No running agents found.");
        } else {
            println!("  No agents found.");
        }
        return Ok(());
    }

    // Display as table
    let json = serde_json::to_value(&agents)?;
    if let Some(arr) = json.as_array() {
        let table = Table::from_json_array(arr);
        println!("{}", table);
    }

    println!();
    println!("  Total: {} agent(s)", agents.len());

    Ok(())
}

/// Format timestamp as human-readable string
fn format_timestamp(millis: u64) -> String {
    use chrono::{DateTime, Local};
    use std::time::UNIX_EPOCH;

    let duration = std::time::Duration::from_millis(millis);
    let datetime = DateTime::<Local>::from(UNIX_EPOCH + duration);
    datetime.format("%Y-%m-%d %H:%M:%S").to_string()
}

#[derive(Debug, Clone, Serialize)]
struct AgentInfo {
    id: String,
    name: String,
    status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    process_id: Option<String>,
    starts: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    last_started: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    description: Option<String>,
}
