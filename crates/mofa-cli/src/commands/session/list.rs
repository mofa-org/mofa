//! `mofa session list` command implementation

use crate::context::CliContext;
use crate::output::Table;
use colored::Colorize;
use serde::Serialize;

/// Execute the `mofa session list` command
pub async fn run(
    ctx: &CliContext,
    agent_id: Option<&str>,
    limit: Option<usize>,
) -> anyhow::Result<()> {
    println!("{} Listing sessions", "→".green());

    if let Some(agent) = agent_id {
        println!("  Filtering by agent: {}", agent.cyan());
    }

    if let Some(n) = limit {
        println!("  Limit: {}", n);
    }

    println!();

    let keys = ctx
        .session_manager
        .list()
        .await
        .map_err(|e| anyhow::anyhow!("Failed to list sessions: {}", e))?;

    if keys.is_empty() {
        println!("  No sessions found.");
        return Ok(());
    }

    let mut sessions = Vec::new();
    for key in &keys {
        let session = ctx.session_manager.get_or_create(key).await;

        // Filter by agent_id if provided (check metadata or key prefix)
        if let Some(agent) = agent_id {
            let matches = session
                .metadata
                .get("agent_id")
                .and_then(|v| v.as_str())
                .map(|v| v == agent)
                .unwrap_or_else(|| session.key.contains(agent));
            if !matches {
                continue;
            }
        }

        sessions.push(SessionInfo {
            session_id: session.key.clone(),
            created_at: session.created_at.format("%Y-%m-%d %H:%M:%S").to_string(),
            message_count: session.len(),
            status: if session.is_empty() {
                "empty".to_string()
            } else {
                "active".to_string()
            },
        });
    }

    // Apply limit
    let limited: Vec<_> = if let Some(n) = limit {
        sessions.into_iter().take(n).collect()
    } else {
        sessions
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
    created_at: String,
    message_count: usize,
    status: String,
}
