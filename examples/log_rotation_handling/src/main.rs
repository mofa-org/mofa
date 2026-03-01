//! Demonstrates log rotation handling in `mofa agent logs --tail`
//!
//! This example:
//! 1. Creates a log file and writes to it
//! 2. Simulates log rotation (truncate/rename)
//! 3. Shows how tail command handles rotation gracefully

use std::time::Duration;
use tempfile::TempDir;
use tokio::fs;
use tokio::io::AsyncWriteExt;
use tokio::time::sleep;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();

    println!("Log Rotation Handling Demo\n");
    println!("This example demonstrates how `mofa agent logs --tail` handles log rotation.\n");

    let temp = TempDir::new()?;
    let data_dir = temp.path().join("data");
    fs::create_dir_all(&data_dir).await?;

    let logs_dir = data_dir.join("logs");
    fs::create_dir_all(&logs_dir).await?;

    let agent_id = "rotating-agent";
    let log_file = logs_dir.join(format!("{}.log", agent_id));

    println!("1. Creating initial log file...");
    let mut file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_file)
        .await?;

    // Write some initial logs
    for i in 1..=10 {
        let entry = format!(
            "[{}] INFO: Pre-rotation log entry {}\n",
            chrono::Local::now().format("%Y-%m-%d %H:%M:%S"),
            i
        );
        file.write_all(entry.as_bytes()).await?;
        file.flush().await?;
        sleep(Duration::from_millis(100)).await;
    }

    println!("   Wrote 10 log entries");
    println!("\n2. Simulating log rotation...");
    println!("   (In production, log rotation tools like logrotate do this)");

    // Close file
    drop(file);

    // Rotate: move old log and create new one
    let rotated_log = logs_dir.join(format!("{}.log.1", agent_id));
    fs::rename(&log_file, &rotated_log).await?;

    println!("   Moved old log to: {}", rotated_log.display());

    // Create new log file
    let mut new_file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_file)
        .await?;

    println!("\n3. Writing to new log file (post-rotation)...");
    for i in 1..=5 {
        let entry = format!(
            "[{}] INFO: Post-rotation log entry {}\n",
            chrono::Local::now().format("%Y-%m-%d %H:%M:%S"),
            i
        );
        new_file.write_all(entry.as_bytes()).await?;
        new_file.flush().await?;
        sleep(Duration::from_millis(100)).await;
    }

    println!("   Wrote 5 new log entries");

    println!("\n4. Testing rotation detection...");
    println!("   The `mofa agent logs --tail` command should:");
    println!("   - Detect when log file is rotated (size decreases)");
    println!("   - Automatically reopen the new log file");
    println!("   - Continue tailing without interruption");

    println!("\n5. To test:");
    println!("   # In one terminal, start tailing:");
    println!("   mofa agent logs {} --tail", agent_id);
    println!("\n   # In another terminal, trigger rotation:");
    println!("   mv {0} {0}.old && touch {0}", log_file.display());
    println!("\n   # The tail command should detect rotation and continue");

    println!("\nRotation handling demo complete!");
    println!("\nKey points:");
    println!("   - Tail command polls file size every 100ms");
    println!("   - Detects rotation when file size decreases");
    println!("   - Automatically reopens new log file");
    println!("   - No manual intervention needed");

    println!("\nLog files:");
    println!("   - Current: {}", log_file.display());
    println!("   - Rotated: {}", rotated_log.display());
    println!("   Press Ctrl+C to exit");
    tokio::signal::ctrl_c().await?;

    Ok(())
}
