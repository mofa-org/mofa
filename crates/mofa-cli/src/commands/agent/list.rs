//! `mofa agent list` command implementation

use crate::CliError;
use crate::context::CliContext;
use crate::output::Table;
use chrono::Utc;
use colored::Colorize;
use serde::Serialize;
use std::collections::BTreeMap;

/// Execute the `mofa agent list` command
pub async fn run(ctx: &CliContext, running_only: bool, _show_all: bool) -> Result<(), CliError> {
    println!("{} Listing agents", "→".green());
    println!();

    let agents_metadata = ctx.agent_registry.list().await;
    let persisted_agents = ctx
        .agent_store
        .list()
        .map_err(|e| CliError::StateError(format!("Failed to list persisted agents: {}", e)))?;

    let mut merged: BTreeMap<String, AgentInfo> = BTreeMap::new();

    // Process live agents from the registry first
    for m in &agents_metadata {
        let status = format!("{:?}", m.state);
        let is_running = is_running_state(&status);
        merged.insert(
            m.id.clone(),
            AgentInfo {
                id: m.id.clone(),
                name: m.name.clone(),
                status,
                is_running,
                uptime: None, // Live registry currently doesn't provide start time easily
                provider: None,
                model: None,
                description: m.description.clone(),
            },
        );
    }

    // Merge in persisted agents
    for (_, entry) in persisted_agents {
        merged.entry(entry.id.clone()).or_insert_with(|| {
            let status = entry.state.clone();
            AgentInfo {
                id: entry.id,
                name: entry.name,
                status: status.clone(),
                is_running: is_running_state(&status),
                uptime: Some(format_duration(Utc::now() - entry.started_at)),
                provider: entry.provider,
                model: entry.model,
                description: entry.description,
            }
        });
    }

    if merged.is_empty() {
        println!("  No agents registered.");
        println!();
        println!(
            "  Use {} to register an agent.",
            "mofa agent start <agent_id>".cyan()
        );
        return Ok(());
    }

    let agents: Vec<AgentInfo> = merged.into_values().collect();

    // Filter based on flags
    let filtered: Vec<_> = if running_only {
        agents.into_iter().filter(|a| a.is_running).collect()
    } else {
        agents
    };

    if filtered.is_empty() {
        println!("  No agents found matching criteria.");
        return Ok(());
    }

    // Display as table
    let json = serde_json::to_value(&filtered)?;
    if let Some(arr) = json.as_array() {
        let table = Table::from_json_array(arr);
        println!("{}", table);
    }

    println!();
    println!("  Total: {} agent(s)", filtered.len());

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

/// Formats a duration into a human-readable string (e.g., "2h 15m", "45s").
fn format_duration(duration: chrono::Duration) -> String {
    let seconds = duration.num_seconds();
    if seconds <= 0 {
        return "0s".to_string();
    }
    if seconds < 60 {
        format!("{}s", seconds)
    } else if seconds < 3600 {
        format!("{}m {}s", seconds / 60, seconds % 60)
    } else {
        let hours = seconds / 3600;
        let mins = (seconds % 3600) / 60;
        format!("{}h {}m", hours, mins)
    }
}

/// Agent information for display purposes.
#[derive(Debug, Clone, Serialize)]
struct AgentInfo {
    id: String,
    name: String,
    status: String,
    #[serde(skip_serializing)]
    is_running: bool,
    uptime: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    provider: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    model: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    description: Option<String>,
}

/// Checks if a status string represents a running or ready agent.
fn is_running_state(status: &str) -> bool {
    let s = status.to_lowercase();
    matches!(
        s.as_str(),
        "running" | "ready" | "executing" | "initializing" | "paused"
    )
}
