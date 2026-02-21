//! `mofa session show` command implementation

use crate::commands::backend::CliBackend;
use colored::Colorize;

/// Execute the `mofa session show` command
pub fn run(session_id: &str, format: Option<&str>) -> anyhow::Result<()> {
    println!("{} Session details: {}", "→".green(), session_id.cyan());
    println!();

    let output_format = format.unwrap_or("text");
    let backend = CliBackend::discover()?;
    let session = backend.get_session(session_id)?;

    match output_format {
        "json" => {
            let json = serde_json::json!({
                "session_id": session.session_id,
                "agent_id": session.agent_id,
                "created_at": session.created_at,
                "messages": session.messages,
                "status": session.status
            });
            println!("{}", serde_json::to_string_pretty(&json)?);
        }
        "yaml" => {
            println!("{}", serde_yaml::to_string(&session)?);
        }
        _ => {
            println!("  Session ID:    {}", session.session_id.cyan());
            println!("  Agent ID:      {}", session.agent_id.white());
            println!("  Created:       {}", session.created_at.white());
            println!("  Status:        {}", session.status.green());
            println!();
            println!("  Messages:");
            for msg in session.messages {
                println!("    {}: {}", msg.role, msg.content);
            }
        }
    }

    Ok(())
}
