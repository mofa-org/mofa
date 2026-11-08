//! Integration tests for AgentProcessManager platform-specific behavior.
//!
//! Covers:
//! - Unix-specific termination paths (SIGTERM graceful, SIGKILL forceful)
//! - Windows-specific termination paths (taskkill graceful vs /F force)
//! - Process status querying (`is_running`)
//! - Non-existent PID and already-terminated process edge cases
//! - Config validation and isolation using tempfile

#![cfg(test)]

use mofa_cli::utils::AgentProcessManager;
use std::process::{Child, Command, Stdio};
use std::time::Duration;
use tempfile::tempdir;

/// Helper to spawn a long-running child process across platforms.
///
/// On Unix, uses POSIX `sleep`.
/// On Windows, uses `timeout`.
#[cfg(unix)]
fn spawn_test_child(duration_secs: u64) -> Child {
    Command::new("sleep")
        .arg(duration_secs.to_string())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("failed to spawn test child process")
}

#[cfg(windows)]
fn spawn_test_child(duration_secs: u64) -> Child {
    Command::new("timeout")
        .args(["/t", &duration_secs.to_string(), "/nobreak"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("failed to spawn test child process")
}

// ============================================================================
// Unix Platform Tests
// ============================================================================

#[cfg(unix)]
#[tokio::test]
async fn test_unix_graceful_termination_sigterm() {
    let temp_dir = tempdir().expect("tempdir");
    let manager = AgentProcessManager::new(temp_dir.path().to_path_buf());

    let mut child = spawn_test_child(60);
    let pid = child.id();

    // Verify process is initially running
    assert!(
        manager.is_running(pid),
        "Spawned child process should be detected as running"
    );

    // Stop process gracefully (force = false -> SIGTERM)
    let stop_result = manager.stop_agent_by_pid(pid, false).await;
    assert!(
        stop_result.is_ok(),
        "Graceful termination via SIGTERM should succeed"
    );

    // Reap the child to avoid leaving a zombie
    let _ = child.wait();

    // Verify process is no longer running
    assert!(
        !manager.is_running(pid),
        "Process should not be reported as running after SIGTERM"
    );
}

#[cfg(unix)]
#[tokio::test]
async fn test_unix_forceful_termination_sigkill() {
    let temp_dir = tempdir().expect("tempdir");
    let manager = AgentProcessManager::new(temp_dir.path().to_path_buf());

    let mut child = spawn_test_child(60);
    let pid = child.id();

    assert!(
        manager.is_running(pid),
        "Spawned child process should be detected as running"
    );

    // Stop process forcefully (force = true -> SIGKILL)
    let stop_result = manager.stop_agent_by_pid(pid, true).await;
    assert!(
        stop_result.is_ok(),
        "Forceful termination via SIGKILL should succeed"
    );

    let _ = child.wait();

    assert!(
        !manager.is_running(pid),
        "Process should not be reported as running after SIGKILL"
    );
}

#[cfg(unix)]
#[tokio::test]
async fn test_unix_terminating_already_exited_process_fails() {
    let temp_dir = tempdir().expect("tempdir");
    let manager = AgentProcessManager::new(temp_dir.path().to_path_buf());

    // Spawn a process that exits immediately
    let mut child = spawn_test_child(0);
    let pid = child.id();

    // Wait for the child to exit and be reaped
    let _ = child.wait();

    // Small delay to ensure OS table cleanup
    tokio::time::sleep(Duration::from_millis(50)).await;

    // Trying to kill an already terminated process should return an error
    let result = manager.stop_agent_by_pid(pid, false).await;
    assert!(
        result.is_err(),
        "Sending signal to already exited process must fail"
    );
}

// ============================================================================
// Windows Platform Tests
// ============================================================================

#[cfg(windows)]
#[tokio::test]
async fn test_windows_graceful_termination() {
    let temp_dir = tempdir().expect("tempdir");
    let manager = AgentProcessManager::new(temp_dir.path().to_path_buf());

    let mut child = spawn_test_child(60);
    let pid = child.id();

    assert!(
        manager.is_running(pid),
        "Spawned Windows process should be detected as running"
    );

    // Stop process gracefully (taskkill /PID <pid>)
    let stop_result = manager.stop_agent_by_pid(pid, false).await;
    assert!(
        stop_result.is_ok(),
        "Windows graceful termination via taskkill should succeed"
    );

    let _ = child.wait();

    assert!(
        !manager.is_running(pid),
        "Process should not be running after taskkill"
    );
}

#[cfg(windows)]
#[tokio::test]
async fn test_windows_forceful_termination() {
    let temp_dir = tempdir().expect("tempdir");
    let manager = AgentProcessManager::new(temp_dir.path().to_path_buf());

    let mut child = spawn_test_child(60);
    let pid = child.id();

    assert!(
        manager.is_running(pid),
        "Spawned Windows process should be detected as running"
    );

    // Stop process forcefully (taskkill /F /PID <pid>)
    let stop_result = manager.stop_agent_by_pid(pid, true).await;
    assert!(
        stop_result.is_ok(),
        "Windows forceful termination via taskkill /F should succeed"
    );

    let _ = child.wait();

    assert!(
        !manager.is_running(pid),
        "Process should not be running after forceful taskkill"
    );
}

// ============================================================================
// Cross-Platform Edge Cases & Configuration Tests
// ============================================================================

#[tokio::test]
async fn test_terminating_invalid_pid_fails() {
    let temp_dir = tempdir().expect("tempdir");
    let manager = AgentProcessManager::new(temp_dir.path().to_path_buf());

    // Non-existent PID
    let invalid_pid = 4_194_300;

    assert!(
        !manager.is_running(invalid_pid),
        "Invalid PID should not be reported as running"
    );

    let result = manager.stop_agent_by_pid(invalid_pid, false).await;
    assert!(
        result.is_err(),
        "Terminating a non-existent PID should return an error"
    );
}

#[test]
fn test_config_validation_lifecycle() {
    let temp_dir = tempdir().expect("tempdir");
    let manager = AgentProcessManager::new(temp_dir.path().to_path_buf());

    // 1. Missing config path should fail validation
    let missing_path = temp_dir.path().join("non_existent_config.yaml");
    let missing_result = manager.validate_config(&missing_path);
    assert!(
        missing_result.is_err(),
        "Missing configuration file must fail validation"
    );

    // 2. Valid YAML configuration should pass
    let valid_path = temp_dir.path().join("agent_valid.yaml");
    std::fs::write(
        &valid_path,
        "agent:\n  name: TestAgent\n  version: '1.0'\n  capabilities:\n    - inference\n",
    )
    .expect("write valid config");
    let valid_result = manager.validate_config(&valid_path);
    assert!(
        valid_result.is_ok(),
        "Well-formed YAML configuration must pass validation"
    );

    // 3. Corrupted / invalid YAML should fail
    let invalid_path = temp_dir.path().join("agent_invalid.yaml");
    std::fs::write(&invalid_path, "agent: { unclosed bracket").expect("write invalid config");
    let invalid_result = manager.validate_config(&invalid_path);
    assert!(
        invalid_result.is_err(),
        "Malformed YAML configuration must fail validation"
    );
}

#[test]
fn test_start_agent_missing_config_fails() {
    let temp_dir = tempdir().expect("tempdir");
    let manager = AgentProcessManager::new(temp_dir.path().to_path_buf());

    let result = manager.start_agent("unknown-agent-id", None, false);
    assert!(
        result.is_err(),
        "Starting agent with non-existent configuration should fail"
    );
}
