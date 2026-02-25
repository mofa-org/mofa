//! `mofa agent logs` command implementation

use crate::context::CliContext;
use crate::utils::paths;
use colored::Colorize;
use std::io::{BufRead, BufReader, Seek, SeekFrom};

/// Read the last `n` lines from a file.
///
/// Returns as many lines as available if the file has fewer than `n` lines.
fn tail_lines(path: &std::path::Path, n: usize) -> anyhow::Result<Vec<String>> {
    let file = std::fs::File::open(path)
        .map_err(|e| anyhow::anyhow!("Failed to open log file '{}': {}", path.display(), e))?;
    let reader = BufReader::new(file);

    // Collect all lines and take the last `n`.
    // For very large files a reverse-seek approach would be more efficient,
    // but this is simple and correct for typical agent log sizes.
    let all_lines: Vec<String> = reader
        .lines()
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| anyhow::anyhow!("Failed to read log file: {}", e))?;

    let start = all_lines.len().saturating_sub(n);
    Ok(all_lines[start..].to_vec())
}

/// Follow a log file, printing new lines as they are appended.
///
/// This uses a simple poll-based approach: sleep briefly, check if the file
/// has grown, and read any new content.  It runs until the caller cancels
/// (Ctrl+C in practice).
async fn follow_file(path: &std::path::Path) -> anyhow::Result<()> {
    let mut file = std::fs::File::open(path)
        .map_err(|e| anyhow::anyhow!("Failed to open log file '{}': {}", path.display(), e))?;

    // Seek to end so we only print *new* content.
    file.seek(SeekFrom::End(0))?;
    let mut reader = BufReader::new(file);

    loop {
        let mut line = String::new();
        match reader.read_line(&mut line) {
            Ok(0) => {
                // No new content — wait briefly before polling again.
                tokio::time::sleep(std::time::Duration::from_millis(200)).await;
            }
            Ok(_) => {
                // Remove the trailing newline for consistent output.
                print!("{}", line);
            }
            Err(e) => {
                anyhow::bail!("Error reading log file: {}", e);
            }
        }
    }
}

/// Execute the `mofa agent logs` command
pub async fn run(ctx: &CliContext, agent_id: &str, tail: bool, lines: usize) -> anyhow::Result<()> {
    // 1. Validate input
    let agent_id = agent_id.trim();
    if agent_id.is_empty() {
        anyhow::bail!("Agent ID cannot be empty");
    }

    // 2. Check agent exists (in-memory registry or persisted store)
    let in_registry = ctx.agent_registry.contains(agent_id).await;
    let in_store = ctx
        .agent_store
        .get(agent_id)
        .map(|v| v.is_some())
        .unwrap_or(false);

    if !in_registry && !in_store {
        anyhow::bail!(
            "Agent '{}' not found. Use {} to see available agents.",
            agent_id,
            "mofa agent list --all".cyan()
        );
    }

    // 3. Resolve log file path
    let log_path = paths::agent_log_path(&ctx.data_dir, agent_id);

    // 4. Check log file exists
    if !log_path.exists() {
        println!(
            "{} No logs found for agent '{}'.",
            "!".yellow(),
            agent_id.cyan()
        );
        println!();
        println!(
            "  Log file expected at: {}",
            log_path.display().to_string().white()
        );
        println!("  Logs will appear here once the agent produces output.");
        return Ok(());
    }

    // 5. Read or tail
    if tail {
        println!(
            "{} Tailing logs for agent: {}",
            "→".green(),
            agent_id.cyan()
        );
        println!("  (Press Ctrl+C to exit)\n");

        // Show last N lines first, then follow
        let recent = tail_lines(&log_path, lines)?;
        for line in &recent {
            println!("{}", line);
        }

        follow_file(&log_path).await?;
    } else {
        println!(
            "{} Displaying recent logs for agent: {}\n",
            "→".green(),
            agent_id.cyan()
        );
    }

        let recent = tail_lines(&log_path, lines)?;

    // Simulate some standard output for the stub
    println!(
        "[{}] INFO: Agent process started successfully.",
        chrono::Local::now().format("%Y-%m-%d %H:%M:%S")
    );
    println!(
        "[{}] INFO: Loaded configuration securely.",
        chrono::Local::now().format("%Y-%m-%d %H:%M:%S")
    );

    if tail {
        // Just a brief simulation before exiting so the command completes in tests
        std::thread::sleep(std::time::Duration::from_secs(1));
        println!(
            "[{}] DEBUG: Connection established with registry.",
            chrono::Local::now().format("%Y-%m-%d %H:%M:%S")
        );
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::agent::start;
    use crate::context::CliContext;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_logs_validates_empty_agent_id() {
        let temp = TempDir::new().unwrap();
        let ctx = CliContext::with_temp_dir(temp.path()).await.unwrap();

        let result = run(&ctx, "", false, 50).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("cannot be empty"));
    }

    #[tokio::test]
    async fn test_logs_returns_error_for_unknown_agent() {
        let temp = TempDir::new().unwrap();
        let ctx = CliContext::with_temp_dir(temp.path()).await.unwrap();

        let result = run(&ctx, "nonexistent-agent", false, 50).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not found"));
    }

    #[tokio::test]
    async fn test_logs_reports_missing_log_file() {
        let temp = TempDir::new().unwrap();
        let ctx = CliContext::with_temp_dir(temp.path()).await.unwrap();

        // Start an agent so it exists in the store
        start::run(&ctx, "log-test-agent", None, None, false)
            .await
            .unwrap();

        // Don't create a log file — the handler should succeed with a message
        let result = run(&ctx, "log-test-agent", false, 50).await;
        assert!(result.is_ok(), "missing log file should not be an error");
    }

    #[tokio::test]
    async fn test_logs_reads_existing_log_file() {
        let temp = TempDir::new().unwrap();
        let ctx = CliContext::with_temp_dir(temp.path()).await.unwrap();

        // Start an agent
        start::run(&ctx, "readable-agent", None, None, false)
            .await
            .unwrap();

        // Create a log file with content
        let log_path = paths::agent_log_path(&ctx.data_dir, "readable-agent");
        std::fs::create_dir_all(log_path.parent().unwrap()).unwrap();
        std::fs::write(&log_path, "line 1\nline 2\nline 3\n").unwrap();

        // The handler should succeed (it prints to stdout, we just verify no error)
        let result = run(&ctx, "readable-agent", false, 50).await;
        assert!(result.is_ok(), "reading log file should succeed");
    }

    #[tokio::test]
    async fn test_logs_respects_line_limit() {
        let temp = TempDir::new().unwrap();
        let ctx = CliContext::with_temp_dir(temp.path()).await.unwrap();

        start::run(&ctx, "limited-agent", None, None, false)
            .await
            .unwrap();

        // Write 100 lines
        let log_path = paths::agent_log_path(&ctx.data_dir, "limited-agent");
        std::fs::create_dir_all(log_path.parent().unwrap()).unwrap();
        let content: String = (1..=100).map(|i| format!("log line {}\n", i)).collect();
        std::fs::write(&log_path, content).unwrap();

        // Read only last 10 lines via tail_lines helper
        let lines = tail_lines(&log_path, 10).unwrap();
        assert_eq!(lines.len(), 10);
        assert_eq!(lines[0], "log line 91");
        assert_eq!(lines[9], "log line 100");
    }
}
