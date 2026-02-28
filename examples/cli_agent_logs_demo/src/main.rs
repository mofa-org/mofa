//! Demonstrates `mofa agent logs` command functionality
//!
//! This example:
//! 1. Creates a test agent that writes logs
//! 2. Shows how to view logs with `mofa agent logs <agent_id>`
//! 3. Demonstrates tailing logs with `--tail` flag
//! 4. Tests log rotation handling

use std::time::Duration;
use tempfile::TempDir;
use tokio::fs;
use tokio::io::AsyncWriteExt;
use tokio::time::sleep;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();

    println!("CLI Agent Logs Demo\n");
    println!("This example demonstrates the `mofa agent logs` command.\n");

    let temp = TempDir::new()?;
    let data_dir = temp.path().join("data");
    fs::create_dir_all(&data_dir).await?;

    // Create logs directory
    let logs_dir = data_dir.join("logs");
    fs::create_dir_all(&logs_dir).await?;

    let agent_id = "demo-agent";
    let log_file = logs_dir.join(format!("{}.log", agent_id));

    println!("1. Creating agent log file: {}", log_file.display());
    
    // Simulate agent writing logs
    let mut file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_file)
        .await?;

    println!("2. Writing initial log entries...");
    for i in 1..=5 {
        let log_entry = format!(
            "[{}] INFO: Agent process started successfully (iteration {})\n",
            chrono::Local::now().format("%Y-%m-%d %H:%M:%S"),
            i
        );
        file.write_all(log_entry.as_bytes()).await?;
        file.flush().await?;
        sleep(Duration::from_millis(200)).await;
    }

    println!("3. Log file created with content. You can now:");
    println!("   - View logs: mofa agent logs {}", agent_id);
    println!("   - Tail logs: mofa agent logs {} --tail", agent_id);
    println!("\n4. Demonstrating log rotation...");
    
    // Simulate log rotation by truncating and writing new content
    drop(file);
    let mut file = fs::OpenOptions::new()
        .write(true)
        .truncate(true)
        .open(&log_file)
        .await?;

    println!("   (Log file rotated - new content will appear)");
    for i in 1..=3 {
        let log_entry = format!(
            "[{}] INFO: Post-rotation log entry {}\n",
            chrono::Local::now().format("%Y-%m-%d %H:%M:%S"),
            i
        );
        file.write_all(log_entry.as_bytes()).await?;
        file.flush().await?;
        sleep(Duration::from_millis(200)).await;
    }

    println!("\nDemo complete!");
    println!("\nTo test the CLI commands:");
    println!("   cd {}", temp.path().display());
    println!("   mofa agent logs {}  # View all logs", agent_id);
    println!("   mofa agent logs {} --tail  # Follow logs in real-time", agent_id);

    // Keep temp dir alive for manual testing
    println!("\nPress Ctrl+C to exit (temp dir will be cleaned up)");
    tokio::signal::ctrl_c().await?;

    Ok(())
}
