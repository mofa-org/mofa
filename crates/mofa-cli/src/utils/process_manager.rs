//! Agent process manager - handles spawning and managing agent runtime processes

use crate::CliError;

type Result<T> = std::result::Result<T, CliError>;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use tracing::{debug, info, warn};

/// Manages agent runtime processes
pub struct AgentProcessManager {
    /// Directory containing agent configurations
    config_dir: PathBuf,
}

impl AgentProcessManager {
    /// Create new process manager
    pub fn new(config_dir: PathBuf) -> Self {
        Self { config_dir }
    }

    /// Start an agent process
    ///
    /// # Arguments
    /// * `agent_id` - Unique agent identifier
    /// * `config_path` - Path to agent configuration file
    /// * `daemon` - If true, run in background; if false, run in foreground
    ///
    /// # Returns
    /// Process ID of the started agent, or error if startup failed
    pub fn start_agent(
        &self,
        agent_id: &str,
        config_path: Option<&Path>,
        daemon: bool,
    ) -> Result<u32> {
        debug!("Starting agent: {} (daemon: {})", agent_id, daemon);

        // Determine config file to use
        let config = if let Some(path) = config_path {
            path.to_path_buf()
        } else {
            // Try to find config in default locations
            let default_path = self.config_dir.join(format!("{}.yaml", agent_id));
            if !default_path.exists() {
                return Err(CliError::StateError(format!(
                    "No configuration found for agent '{}' at {}",
                    agent_id,
                    default_path.display()
                )));
            }
            default_path
        };

        // Verify config file exists
        if !config.exists() {
            return Err(CliError::StateError(format!("Agent configuration not found at: {}", config.display())));
        }

        info!(
            "Starting agent '{}' with config: {}",
            agent_id,
            config.display()
        );

        // Build the command to run the agent
        let mut cmd = Command::new("cargo");
        cmd.arg("run")
            .arg("-p")
            .arg("mofa-cli")
            .arg("--")
            .arg("run")
            .arg(config.to_string_lossy().as_ref());

        // Configure output
        if daemon {
            cmd.stdout(Stdio::null())
                .stderr(Stdio::null())
                .stdin(Stdio::null());
        } else {
            cmd.stdout(Stdio::inherit())
                .stderr(Stdio::inherit())
                .stdin(Stdio::inherit());
        }

        // Spawn the process
        let mut child = cmd
            .spawn()
            .map_err(|e| CliError::StateError(format!("Failed to start agent '{}' process: {}", agent_id, e)))?;

        // Get the child process ID
        let pid = child.id();
        info!("Agent '{}' started with PID: {}", agent_id, pid);

        // For daemon mode, we can let the process run independently
        // For foreground mode, we wait for completion
        if daemon {
            // Detach from parent process in daemon mode
            // On Unix, this is automatic; on Windows, we just let it go
            std::mem::drop(child);
        }

        Ok(pid)
    }

    /// Stop an agent process by PID
    ///
    /// # Arguments
    /// * `pid` - Process ID to terminate
    /// * `force` - If true, use SIGKILL; if false, try SIGTERM first
    pub async fn stop_agent_by_pid(&self, pid: u32, force: bool) -> Result<()> {
        debug!("Stopping agent with PID: {} (force: {})", pid, force);

        #[cfg(unix)]
        {
            use nix::sys::signal::{Signal, kill};
            use nix::unistd::Pid;

            let nix_pid = Pid::from_raw(pid as i32);
            let signal = if force {
                Signal::SIGKILL
            } else {
                Signal::SIGTERM
            };

            match kill(nix_pid, Some(signal)) {
                Ok(_) => {
                    info!("Sent {:?} to process {}", signal, pid);
                    Ok(())
                }
                Err(e) => {
                    warn!("Failed to send {:?} to process {}: {}", signal, pid, e);
                    Err(CliError::StateError(format!(
                        "Failed to terminate process {}: {}",
                        pid,
                        e
                    )))
                }
            }
        }

        #[cfg(windows)]
        {
            use std::process::Command;

            // On Windows, use taskkill command
            let status = Command::new("taskkill")
                .arg(if force { "/F" } else { "" })
                .arg("/PID")
                .arg(pid.to_string())
                .status()?;

            if status.success() {
                info!("Successfully terminated process {}", pid);
                Ok(())
            } else {
                return Err(CliError::StateError(format!("Failed to terminate process {}", pid)));
            }
        }

        #[cfg(not(any(unix, windows)))]
        {
            return Err(CliError::StateError("Agent process termination not supported on this platform".to_string()));
        }
    }

    /// Check if a process is running
    pub fn is_running(&self, pid: u32) -> bool {
        #[cfg(unix)]
        {
            use nix::sys::signal::kill;
            use nix::unistd::Pid;

            let nix_pid = Pid::from_raw(pid as i32);
            // Signal 0 is used to check if process exists without sending a signal
            kill(nix_pid, None).is_ok()
        }

        #[cfg(windows)]
        {
            use std::process::Command;

            let output = Command::new("tasklist")
                .arg("/FI")
                .arg(format!("PID eq {}", pid))
                .output();

            output
                .map(|o| String::from_utf8_lossy(&o.stdout).contains(&pid.to_string()))
                .unwrap_or(false)
        }

        #[cfg(not(any(unix, windows)))]
        {
            false
        }
    }

    /// Validate agent configuration
    pub fn validate_config(&self, config_path: &Path) -> Result<()> {
        if !config_path.exists() {
            return Err(CliError::StateError(format!("Configuration file not found: {}", config_path.display())));
        }

        // Try to load and parse as YAML
        let content = std::fs::read_to_string(config_path)?;
        serde_yaml::from_str::<serde_yaml::Value>(&content)
            .map_err(|e| CliError::StateError(format!("Invalid YAML in config: {}", e)))?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_process_manager_creation() {
        let temp_dir = TempDir::new().unwrap();
        let manager = AgentProcessManager::new(temp_dir.path().to_path_buf());

        assert!(manager.config_dir.exists());
    }

    #[test]
    fn test_validate_config_missing_file() {
        let temp_dir = TempDir::new().unwrap();
        let manager = AgentProcessManager::new(temp_dir.path().to_path_buf());

        let result = manager.validate_config(std::path::Path::new("/nonexistent/config.yaml"));
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_config_valid_yaml() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("config.yaml");
        std::fs::write(&config_path, "agent:\n  name: Test\n  id: test-1\n").unwrap();

        let manager = AgentProcessManager::new(temp_dir.path().to_path_buf());
        let result = manager.validate_config(&config_path);
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_config_invalid_yaml() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("config.yaml");
        std::fs::write(&config_path, "agent:\n  name: [unclosed").unwrap();

        let manager = AgentProcessManager::new(temp_dir.path().to_path_buf());
        let result = manager.validate_config(&config_path);
        assert!(result.is_err());
    }
}
