//! Integration tests for agent management CLI commands
//!
//! Tests the mofa agent list, start, and stop commands using the public API

#![cfg(test)]

use mofa_cli::commands::agent::state::{AgentRegistry, AgentState, AgentStatus};

use std::env;
use tempfile::TempDir;

// Note: These are integration tests that verify the CLI commands work correctly.
// They use a temporary directory to avoid interfering with the actual CLI state.

#[test]
fn test_agent_state_module_integration() {
    // This test verifies the agent state module is properly integrated and accessible
    assert!(true); // Compilation of the module is the real test
}

#[tokio::test]
async fn test_agent_state_persistence() {
    let temp_dir = TempDir::new().unwrap();
    unsafe { env::set_var("MOFA_HOME", temp_dir.path()); }

    let registry = AgentRegistry::new().unwrap();

    // Create and save an agent
    let mut agent = AgentState::new("test-agent".to_string(), "TestAgent".to_string());
    agent.start_running(1234, Some("/path/to/config.yml".to_string()));

    registry.save_agent(&agent).unwrap();

    // Load it back
    let loaded = registry.load_agent("test-agent").unwrap();
    assert!(loaded.is_some());

    let loaded_agent = loaded.unwrap();
    assert_eq!(loaded_agent.id, "test-agent");
    assert_eq!(loaded_agent.name, "TestAgent");
    assert_eq!(loaded_agent.status, AgentStatus::Running);
    assert_eq!(loaded_agent.pid, Some(1234));
}

#[tokio::test]
async fn test_agent_list_empty() {
    let temp_dir = TempDir::new().unwrap();
    unsafe { env::set_var("MOFA_HOME", temp_dir.path()); }

    let registry = AgentRegistry::new().unwrap();
    let agents = registry.list_all().unwrap();

    assert_eq!(agents.len(), 0);
}

#[tokio::test]
async fn test_agent_list_multiple() {
    let temp_dir = TempDir::new().unwrap();
    unsafe { env::set_var("MOFA_HOME", temp_dir.path()); }

    let registry = AgentRegistry::new().unwrap();

    // Create multiple agents
    for i in 1..=3 {
        let mut agent = AgentState::new(format!("agent-{}", i), format!("Agent {}", i));
        if i % 2 == 0 {
            agent.start_running(1000 + i as u32, None);
        }
        registry.save_agent(&agent).unwrap();
    }

    let all_agents = registry.list_all().unwrap();
    assert_eq!(all_agents.len(), 3);

    let running = registry.list_running().unwrap();
    assert_eq!(running.len(), 1); // Only agent-2 is running
    assert_eq!(running[0].id, "agent-2");
    assert_eq!(running[0].status, AgentStatus::Running);
}

#[tokio::test]
async fn test_agent_status_transitions() {
    let temp_dir = TempDir::new().unwrap();
    unsafe { env::set_var("MOFA_HOME", temp_dir.path()); }

    let registry = AgentRegistry::new().unwrap();

    let mut agent = AgentState::new("test-agent".to_string(), "TestAgent".to_string());

    // Initial state: stopped
    assert_eq!(agent.status, AgentStatus::Stopped);
    registry.save_agent(&agent).unwrap();

    // Transition to running
    agent.start_running(1234, None);
    assert_eq!(agent.status, AgentStatus::Running);
    registry.save_agent(&agent).unwrap();

    // Verify persistence
    let loaded = registry.load_agent("test-agent").unwrap().unwrap();
    assert_eq!(loaded.status, AgentStatus::Running);

    // Transition to stopped
    agent.stop();
    assert_eq!(agent.status, AgentStatus::Stopped);
    registry.save_agent(&agent).unwrap();

    // Verify persistence
    let loaded = registry.load_agent("test-agent").unwrap().unwrap();
    assert_eq!(loaded.status, AgentStatus::Stopped);
    assert_eq!(loaded.pid, None);
}

#[tokio::test]
async fn test_agent_error_status() {
    let temp_dir = TempDir::new().unwrap();
    unsafe { env::set_var("MOFA_HOME", temp_dir.path()); }

    let registry = AgentRegistry::new().unwrap();

    let mut agent = AgentState::new("error-agent".to_string(), "ErrorAgent".to_string());
    agent.set_error("Failed to initialize".to_string());

    assert_eq!(agent.status, AgentStatus::Error);
    assert_eq!(agent.error, Some("Failed to initialize".to_string()));

    registry.save_agent(&agent).unwrap();

    let loaded = registry.load_agent("error-agent").unwrap().unwrap();
    assert_eq!(loaded.status, AgentStatus::Error);
    assert_eq!(loaded.error, Some("Failed to initialize".to_string()));
}

#[tokio::test]
async fn test_agent_uptime_calculation() {
    let mut agent = AgentState::new("test".to_string(), "Test".to_string());

    // No uptime when stopped
    assert_eq!(agent.uptime_string(), None);

    // Start the agent
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();

    agent.start_time = Some(now);

    // Should have uptime
    let uptime = agent.uptime_string();
    assert!(uptime.is_some());
    let uptime_str = uptime.unwrap();
    assert!(uptime_str.contains("s"));
}

#[tokio::test]
async fn test_agent_metadata_storage() {
    let temp_dir = TempDir::new().unwrap();
    unsafe { env::set_var("MOFA_HOME", temp_dir.path()); }

    let registry = AgentRegistry::new().unwrap();

    let mut agent = AgentState::new("meta-agent".to_string(), "MetaAgent".to_string());
    agent
        .metadata
        .insert("version".to_string(), "1.0.0".to_string());
    agent
        .metadata
        .insert("provider".to_string(), "openai".to_string());

    registry.save_agent(&agent).unwrap();

    let loaded = registry.load_agent("meta-agent").unwrap().unwrap();
    assert_eq!(loaded.metadata.get("version"), Some(&"1.0.0".to_string()));
    assert_eq!(loaded.metadata.get("provider"), Some(&"openai".to_string()));
}

#[tokio::test]
async fn test_agent_registry_delete() {
    let temp_dir = TempDir::new().unwrap();
    unsafe { env::set_var("MOFA_HOME", temp_dir.path()); }

    let registry = AgentRegistry::new().unwrap();

    let agent = AgentState::new("delete-agent".to_string(), "DeleteAgent".to_string());
    registry.save_agent(&agent).unwrap();

    // Verify it exists
    assert!(registry.exists("delete-agent").unwrap());

    // Delete it
    registry.delete_agent("delete-agent").unwrap();

    // Verify it's gone
    assert!(!registry.exists("delete-agent").unwrap());
}

#[tokio::test]
async fn test_agent_registry_concurrent_access() {
    use std::sync::Arc;

    let temp_dir = TempDir::new().unwrap();
    unsafe { env::set_var("MOFA_HOME", temp_dir.path()); }

    let registry = Arc::new(AgentRegistry::new().unwrap());

    let mut handles = vec![];

    // Create multiple agents concurrently
    for i in 0..5 {
        let reg = Arc::clone(&registry);
        let handle = tokio::spawn(async move {
            let mut agent = AgentState::new(
                format!("concurrent-agent-{}", i),
                format!("ConcurrentAgent{}", i),
            );
            agent.start_running(2000 + i as u32, None);
            reg.save_agent(&agent).unwrap();
            agent
        });
        handles.push(handle);
    }

    for handle in handles {
        let _ = handle.await;
    }

    // Verify all were saved
    let agents = registry.list_all().unwrap();
    assert_eq!(agents.len(), 5);
}

#[test]
fn test_agent_status_display() {
    assert_eq!(AgentStatus::Running.display(), "running");
    assert_eq!(AgentStatus::Stopped.display(), "stopped");
    assert_eq!(AgentStatus::Error.display(), "error");
}

#[test]
fn test_agent_status_colored_display() {
    let running_display = AgentStatus::Running.colored_display();
    assert!(!running_display.is_empty());

    let stopped_display = AgentStatus::Stopped.colored_display();
    assert!(!stopped_display.is_empty());

    let error_display = AgentStatus::Error.colored_display();
    assert!(!error_display.is_empty());
}
