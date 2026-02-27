//! `mofa session show` command implementation

use crate::CliError;
use crate::context::CliContext;
use colored::Colorize;

/// Execute the `mofa session show` command
pub async fn run(ctx: &CliContext, session_id: &str, format: Option<&str>) -> Result<(), CliError> {
    println!("{} Session details: {}", "→".green(), session_id.cyan());
    println!();

    let session = ctx
        .session_manager
        .get(session_id)
        .await
        .map_err(|e| CliError::SessionError(format!("Failed to load session: {}", e)))?
        .ok_or_else(|| CliError::SessionError(format!("Session '{}' not found", session_id)))?;
    let output_format = format.unwrap_or("text");

    match output_format {
        "json" => {
            let json = serde_json::json!({
                "session_id": session.key,
                "created_at": session.created_at.to_rfc3339(),
                "updated_at": session.updated_at.to_rfc3339(),
                "message_count": session.len(),
                "metadata": session.metadata,
                "messages": session.messages.iter().map(|m| {
                    serde_json::json!({
                        "role": m.role,
                        "content": m.content,
                        "timestamp": m.timestamp.to_rfc3339(),
                    })
                }).collect::<Vec<_>>(),
            });
            println!("{}", serde_json::to_string_pretty(&json)?);
        }
        "yaml" => {
            let yaml = serde_json::json!({
                "session_id": session.key,
                "created_at": session.created_at.to_rfc3339(),
                "updated_at": session.updated_at.to_rfc3339(),
                "message_count": session.len(),
                "metadata": session.metadata,
                "messages": session.messages.iter().map(|m| {
                    serde_json::json!({
                        "role": m.role,
                        "content": m.content,
                        "timestamp": m.timestamp.to_rfc3339(),
                    })
                }).collect::<Vec<_>>(),
            });
            println!("{}", serde_yaml::to_string(&yaml)?);
        }
        _ => {
            println!("  Session ID:    {}", session.key.cyan());
            println!(
                "  Created:       {}",
                session
                    .created_at
                    .format("%Y-%m-%d %H:%M:%S")
                    .to_string()
                    .white()
            );
            println!(
                "  Updated:       {}",
                session
                    .updated_at
                    .format("%Y-%m-%d %H:%M:%S")
                    .to_string()
                    .white()
            );
            println!("  Messages:      {}", session.len());
            if !session.metadata.is_empty() {
                println!("  Metadata:      {:?}", session.metadata);
            }
            println!();

            if session.is_empty() {
                println!("  (no messages)");
            } else {
                println!("  Messages:");
                for msg in &session.messages {
                    let role_display = match msg.role.as_str() {
                        "user" => "User".green(),
                        "assistant" => "Assistant".cyan(),
                        "system" => "System".yellow(),
                        other => other.white(),
                    };
                    println!("    {}: {}", role_display, msg.content);
                }
            }
        }
    }

    Ok(())
}
