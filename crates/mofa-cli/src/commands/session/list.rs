//! `mofa session list` command implementation

use crate::output::Table;
use colored::Colorize;
use serde::Serialize;

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

    // TODO: Implement actual session listing from persistence layer

    let sessions = vec![
        SessionInfo {
            session_id: "sess-001".to_string(),
            agent_id: "agent-001".to_string(),
            created_at: "2024-01-15 10:30:00".to_string(),
            message_count: 12,
            status: "active".to_string(),
        },
        SessionInfo {
            session_id: "sess-002".to_string(),
            agent_id: "agent-001".to_string(),
            created_at: "2024-01-15 09:15:00".to_string(),
            message_count: 8,
            status: "active".to_string(),
        },
    ];

    let filtered: Vec<_> = if let Some(agent) = agent_id {
        sessions
            .iter()
            .filter(|s| s.agent_id == agent)
            .cloned()
            .collect()
    } else {
        sessions
    };

    let limited: Vec<_> = if let Some(n) = limit {
        filtered.into_iter().take(n).collect()
    } else {
        filtered
    };

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

#[derive(Debug, Clone, Serialize)]
struct SessionInfo {
    session_id: String,
    agent_id: String,
    created_at: String,
    message_count: usize,
    status: String,
}
