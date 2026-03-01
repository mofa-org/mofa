//! `mofa session list` command implementation

use crate::CliError;
use crate::context::CliContext;
use crate::output::Table;
use colored::Colorize;
use serde::Serialize;

/// Execute the `mofa session list` command
pub async fn run(
    ctx: &CliContext,
    agent_id: Option<&str>,
    limit: Option<usize>,
) -> Result<(), CliError> {
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
        .map_err(|e| CliError::SessionError(format!("Failed to list sessions: {}", e)))?;

    if keys.is_empty() {
        println!("  No sessions found.");
        return Ok(());
    }

    let mut sessions = Vec::new();
    for key in &keys {
        let session = match ctx
            .session_manager
            .get(key)
            .await
            .map_err(|e| CliError::SessionError(format!("Failed to load session '{}': {}", key, e)))?
        {
            Some(session) => session,
            None => continue,
        };

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::CliContext;
    use mofa_foundation::agent::session::Session;
    use serde_json::json;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_session_list_runs_with_saved_sessions() {
        let temp = TempDir::new().unwrap();
        let ctx = CliContext::with_temp_dir(temp.path()).await.unwrap();

        let mut session_a = Session::new("agent-a:1");
        session_a
            .metadata
            .insert("agent_id".to_string(), json!("agent-a"));
        session_a.add_message("user", "hello");
        ctx.session_manager.save(&session_a).await.unwrap();

        let mut session_b = Session::new("agent-b:1");
        session_b
            .metadata
            .insert("agent_id".to_string(), json!("agent-b"));
        ctx.session_manager.save(&session_b).await.unwrap();

        run(&ctx, None, None).await.unwrap();
        run(&ctx, Some("agent-a"), None).await.unwrap();
        run(&ctx, Some("agent-b"), Some(1)).await.unwrap();
    }
}
