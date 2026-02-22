//! `mofa session export` command implementation

use colored::Colorize;
use std::path::PathBuf;

/// Execute the `mofa session export` command
pub fn run(session_id: &str, output: PathBuf, format: &str) -> anyhow::Result<()> {
    println!("{} Exporting session: {}", "→".green(), session_id.cyan());
    println!("  Format: {}", format.yellow());
    println!("  Output: {}", output.display().to_string().cyan());
    println!();

    // TODO: Implement actual session export from persistence layer

    let output_str = match format {
        "json" => {
            let content = serde_json::json!({
                "session_id": session_id,
                "agent_id": "agent-001",
                "created_at": "2024-01-15T10:30:00Z",
                "messages": [
                    {"role": "user", "content": "Hello!"},
                    {"role": "assistant", "content": "Hi there! How can I help you?"}
                ],
                "status": "active"
            });
            serde_json::to_string_pretty(&content)?
        }
        "yaml" => {
            format!(
                "session_id: {}\nagent_id: agent-001\ncreated_at: 2024-01-15T10:30:00Z\nmessages:\n  - role: user\n    content: Hello!\n  - role: assistant\n    content: Hi there! How can I help you?\nstatus: active\n",
                session_id
            )
        }
        _ => anyhow::bail!("Unsupported export format: {}", format),
    };

    std::fs::write(&output, output_str)?;
    println!("{} Session exported to {}", "✓".green(), output.display());

    Ok(())
}
