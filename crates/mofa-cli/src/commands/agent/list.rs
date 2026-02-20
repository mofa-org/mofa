//! `mofa agent list` command implementation

use crate::output::Table;
use colored::Colorize;
use serde::Serialize;

/// Execute the `mofa agent list` command
pub fn run(running_only: bool, show_all: bool) -> anyhow::Result<()> {
    println!("{} Listing agents", "→".green());

    if running_only {
        println!("  Showing running agents only");
    } else if show_all {
        println!("  Showing all agents");
    }

    println!();

    // TODO: Implement actual agent listing from state store
    // For now, show example output

    let agents = vec![
        AgentInfo {
            id: "agent-001".to_string(),
            name: "MyAgent".to_string(),
            status: "running".to_string(),
            uptime: Some("5m 32s".to_string()),
            provider: Some("openai".to_string()),
            model: Some("gpt-4o".to_string()),
        },
        AgentInfo {
            id: "agent-002".to_string(),
            name: "TestAgent".to_string(),
            status: "stopped".to_string(),
            uptime: None,
            provider: None,
            model: None,
        },
    ];

    // Filter based on flags
    let filtered: Vec<_> = if running_only {
        agents
            .iter()
            .filter(|a| a.status == "running")
            .cloned()
            .collect()
    } else {
        agents
    };

    if filtered.is_empty() {
        println!("  No agents found.");
        return Ok(());
    }

    // Display as table
    let json = serde_json::to_value(&filtered)?;
    if let Some(arr) = json.as_array() {
        let table = Table::from_json_array(arr);
        println!("{}", table);
    }

    Ok(())
}

#[derive(Debug, Clone, Serialize)]
struct AgentInfo {
    id: String,
    name: String,
    status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    uptime: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    provider: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    model: Option<String>,
}
