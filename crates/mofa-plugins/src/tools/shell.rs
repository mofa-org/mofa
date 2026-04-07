use super::*;
use serde_json::json;
use std::path::{Path, PathBuf};
use tokio::time::{timeout, Duration};
use tokio::process::Command;

#[cfg(not(target_os = "linux"))]
use crate::wasm_runtime::{ResourceLimits, RuntimeConfig, WasmPluginConfig, WasmRuntime};

/// Shell 命令工具 - 执行系统命令（受限）
/// Shell command tool - Execute system commands (restricted)
pub struct ShellCommandTool {
    definition: ToolDefinition,
    allowed_commands: Vec<String>,
    sandbox: ShellSandboxConfig,
}

/// Shell sandbox execution mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShellExecutionMode {
    Host,
    Sandbox,
}

/// Shell sandbox settings.
#[derive(Debug, Clone)]
pub struct ShellSandboxConfig {
    pub mode: ShellExecutionMode,
    pub allowed_working_dirs: Vec<PathBuf>,
    pub timeout_ms: u64,
    pub memory_limit_mb: Option<u64>,
    pub cpu_time_limit_secs: Option<u64>,
    pub clear_env: bool,
    pub max_output_chars: usize,
}

impl Default for ShellSandboxConfig {
    fn default() -> Self {
        Self {
            mode: ShellExecutionMode::Host,
            allowed_working_dirs: Vec::new(),
            timeout_ms: 30_000,
            memory_limit_mb: Some(256),
            cpu_time_limit_secs: Some(5),
            clear_env: true,
            max_output_chars: 5000,
        }
    }
}

impl ShellCommandTool {
    pub fn new(allowed_commands: Vec<String>) -> Self {
        Self::new_with_config(allowed_commands, ShellSandboxConfig::default())
    }

    pub fn new_with_config(
        allowed_commands: Vec<String>,
        sandbox: ShellSandboxConfig,
    ) -> Self {
        Self {
            definition: ToolDefinition {
                name: "shell".to_string(),
                description:
                    "Execute shell commands. Supports optional sandbox mode and command whitelist."
                        .to_string(),
                parameters: json!({
                    "type": "object",
                    "properties": {
                        "command": {
                            "type": "string",
                            "description": "The command to execute"
                        },
                        "args": {
                            "type": "array",
                            "items": { "type": "string" },
                            "description": "Command arguments"
                        },
                        "working_dir": {
                            "type": "string",
                            "description": "Working directory for command execution"
                        },
                        "sandbox": {
                            "type": "boolean",
                            "description": "Optional per-call override for sandbox mode"
                        }
                    },
                    "required": ["command"]
                }),
                requires_confirmation: true,
            },
            allowed_commands,
            sandbox,
        }
    }

    /// Create from tool config map in agent.yml tools[].config
    pub fn new_with_defaults_from_config(config: &serde_json::Value) -> Self {
        let sandbox_enabled = config
            .get("sandbox")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        let allowed_working_dirs = config
            .get("allowed_working_dirs")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str())
                    .map(PathBuf::from)
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();

        let timeout_ms = config
            .get("timeout_ms")
            .and_then(|v| v.as_u64())
            .unwrap_or(30_000);

        let clear_env = config
            .get("clear_env")
            .and_then(|v| v.as_bool())
            .unwrap_or(true);

        let memory_limit_mb = config
            .get("memory_limit_mb")
            .and_then(|v| v.as_u64())
            .or(Some(256));

        let cpu_time_limit_secs = config
            .get("cpu_time_limit_secs")
            .and_then(|v| v.as_u64())
            .or(Some(5));

        let max_output_chars = config
            .get("max_output_chars")
            .and_then(|v| v.as_u64())
            .and_then(|v| usize::try_from(v).ok())
            .unwrap_or(5000);

        let mode = if sandbox_enabled {
            ShellExecutionMode::Sandbox
        } else {
            ShellExecutionMode::Host
        };

        let sandbox = ShellSandboxConfig {
            mode,
            allowed_working_dirs,
            timeout_ms,
            memory_limit_mb,
            cpu_time_limit_secs,
            clear_env,
            max_output_chars,
        };

        Self::new_with_config(Self::default_allowed_commands(), sandbox)
    }

    /// Create with default allowed commands
    pub fn new_with_defaults() -> Self {
        Self::new(Self::default_allowed_commands())
    }

    fn default_allowed_commands() -> Vec<String> {
        vec![
            "ls".to_string(),
            "pwd".to_string(),
            "echo".to_string(),
            "date".to_string(),
            "whoami".to_string(),
            "cat".to_string(),
            "head".to_string(),
            "tail".to_string(),
            "wc".to_string(),
            "grep".to_string(),
            "find".to_string(),
        ]
    }

    fn is_command_allowed(&self, command: &str) -> bool {
        if self.allowed_commands.is_empty() {
            return false; // Default deny if no whitelist
        }
        self.allowed_commands
            .iter()
            .any(|allowed| command == allowed || command.starts_with(&format!("{} ", allowed)))
    }

    fn truncate_utf8(s: String, limit: usize) -> String {
        if s.len() > limit {
            let mut end = limit;
            while !s.is_char_boundary(end) {
                end -= 1;
            }
            format!("{}...[truncated]", &s[..end])
        } else {
            s
        }
    }

    fn resolve_working_dir(&self, arguments: &serde_json::Value) -> PluginResult<PathBuf> {
        let candidate = arguments
            .get("working_dir")
            .and_then(|d| d.as_str())
            .map(PathBuf::from)
            .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));

        let resolved = std::fs::canonicalize(&candidate).map_err(|e| {
            mofa_kernel::plugin::PluginError::ExecutionFailed(format!(
                "Failed to resolve working_dir '{}': {}",
                candidate.display(),
                e
            ))
        })?;

        if self.sandbox.mode == ShellExecutionMode::Sandbox {
            self.ensure_working_dir_allowed(&resolved)?;
        }

        Ok(resolved)
    }

    fn ensure_working_dir_allowed(&self, working_dir: &Path) -> PluginResult<()> {
        if self.sandbox.allowed_working_dirs.is_empty() {
            return Ok(());
        }

        let allowed = self
            .sandbox
            .allowed_working_dirs
            .iter()
            .filter_map(|d| std::fs::canonicalize(d).ok())
            .any(|allowed| working_dir.starts_with(&allowed));

        if allowed {
            Ok(())
        } else {
            Err(mofa_kernel::plugin::PluginError::ExecutionFailed(format!(
                "Working directory '{}' is outside sandbox allowed_working_dirs",
                working_dir.display()
            )))
        }
    }

    fn sandbox_enabled_for_call(&self, arguments: &serde_json::Value) -> bool {
        // Security policy: a tool configured in sandbox mode cannot be downgraded
        // to host execution by per-call arguments.
        if self.sandbox.mode == ShellExecutionMode::Sandbox {
            return true;
        }

        arguments
            .get("sandbox")
            .and_then(|v| v.as_bool())
            .unwrap_or(false)
    }

    async fn execute_host(
        &self,
        command: &str,
        args: &[String],
        working_dir: &Path,
    ) -> PluginResult<std::process::Output> {
        let mut cmd = Command::new(command);
        cmd.args(args).current_dir(working_dir);
        if self.sandbox.clear_env {
            cmd.env_clear();
            cmd.env("PATH", "/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin");
        }

        timeout(
            Duration::from_millis(self.sandbox.timeout_ms),
            cmd.output(),
        )
        .await
        .map_err(|_| {
            mofa_kernel::plugin::PluginError::ExecutionFailed(format!(
                "Command timed out after {}ms",
                self.sandbox.timeout_ms
            ))
        })?
        .map_err(Into::into)
    }

    async fn execute_sandboxed(
        &self,
        command: &str,
        args: &[String],
        working_dir: &Path,
    ) -> PluginResult<std::process::Output> {
        #[cfg(target_os = "linux")]
        {
            if which::which("bwrap").is_err() {
                return Err(mofa_kernel::plugin::PluginError::ExecutionFailed(
                    "Sandbox mode requires 'bwrap' (bubblewrap) on Linux".to_string(),
                ));
            }

            let mut wrapped_args: Vec<String> = vec![
                "--die-with-parent".to_string(),
                "--new-session".to_string(),
                "--unshare-all".to_string(),
                "--clearenv".to_string(),
                "--proc".to_string(),
                "/proc".to_string(),
                "--dev".to_string(),
                "/dev".to_string(),
                "--ro-bind".to_string(),
                "/usr".to_string(),
                "/usr".to_string(),
                "--ro-bind".to_string(),
                "/bin".to_string(),
                "/bin".to_string(),
                "--ro-bind".to_string(),
                "/lib".to_string(),
                "/lib".to_string(),
                "--ro-bind".to_string(),
                "/lib64".to_string(),
                "/lib64".to_string(),
                "--tmpfs".to_string(),
                "/tmp".to_string(),
                "--bind".to_string(),
                working_dir.display().to_string(),
                "/workspace".to_string(),
                "--chdir".to_string(),
                "/workspace".to_string(),
                "--setenv".to_string(),
                "PATH".to_string(),
                "/usr/bin:/bin".to_string(),
                command.to_string(),
            ];
            wrapped_args.extend(args.iter().cloned());

            let use_prlimit = self.sandbox.memory_limit_mb.is_some()
                || self.sandbox.cpu_time_limit_secs.is_some();

            let mut cmd = if use_prlimit {
                if which::which("prlimit").is_err() {
                    return Err(mofa_kernel::plugin::PluginError::ExecutionFailed(
                        "Sandbox resource limits require 'prlimit' on Linux".to_string(),
                    ));
                }

                let mut c = Command::new("prlimit");
                if let Some(memory_mb) = self.sandbox.memory_limit_mb {
                    let memory_bytes = memory_mb.saturating_mul(1024).saturating_mul(1024);
                    c.arg(format!("--as={}", memory_bytes));
                }
                if let Some(cpu_secs) = self.sandbox.cpu_time_limit_secs {
                    c.arg(format!("--cpu={}", cpu_secs));
                }
                c.arg("--").arg("bwrap");
                c
            } else {
                Command::new("bwrap")
            };

            cmd.args(&wrapped_args);

            return timeout(
                Duration::from_millis(self.sandbox.timeout_ms),
                cmd.output(),
            )
            .await
            .map_err(|_| {
                mofa_kernel::plugin::PluginError::ExecutionFailed(format!(
                    "Sandboxed command timed out after {}ms",
                    self.sandbox.timeout_ms
                ))
            })?
            .map_err(Into::into);
        }

        #[cfg(not(target_os = "linux"))]
        {
            self.execute_wasm_fallback(command, args, working_dir).await
        }
    }

    #[cfg(not(target_os = "linux"))]
    async fn execute_wasm_fallback(
        &self,
        command: &str,
        _args: &[String],
        working_dir: &Path,
    ) -> PluginResult<std::process::Output> {
        let path = if Path::new(command).is_absolute() {
            PathBuf::from(command)
        } else {
            working_dir.join(command)
        };

        if path.extension().and_then(|e| e.to_str()) != Some("wasm") {
            return Err(mofa_kernel::plugin::PluginError::ExecutionFailed(
                "Non-Linux sandbox fallback only supports .wasm modules".to_string(),
            ));
        }

        let bytes = tokio::fs::read(&path).await.map_err(|e| {
            mofa_kernel::plugin::PluginError::ExecutionFailed(format!(
                "Failed to read wasm module '{}': {}",
                path.display(),
                e
            ))
        })?;

        let mut limits = ResourceLimits::restrictive();
        if let Some(memory_mb) = self.sandbox.memory_limit_mb {
            let pages = (memory_mb.saturating_mul(1024).saturating_mul(1024)) / 65_536;
            limits.max_memory_pages = u32::try_from(pages).unwrap_or(u32::MAX);
        }
        if let Some(cpu_secs) = self.sandbox.cpu_time_limit_secs {
            limits.max_execution_time_ms = cpu_secs.saturating_mul(1000);
        }

        let runtime = WasmRuntime::new(RuntimeConfig::new().with_resource_limits(limits))
            .map_err(|e| mofa_kernel::plugin::PluginError::ExecutionFailed(e.to_string()))?;

        let mut plugin_config = WasmPluginConfig::new("shell-wasm-fallback");
        plugin_config.allowed_capabilities = Vec::new();

        let plugin = runtime
            .create_plugin_from_bytes(&bytes, plugin_config)
            .await
            .map_err(|e| mofa_kernel::plugin::PluginError::ExecutionFailed(e.to_string()))?;

        plugin
            .initialize()
            .await
            .map_err(|e| mofa_kernel::plugin::PluginError::ExecutionFailed(e.to_string()))?;

        if plugin.has_export("_start").await {
            plugin
                .call_void("_start", &[])
                .await
                .map_err(|e| mofa_kernel::plugin::PluginError::ExecutionFailed(e.to_string()))?;
        } else if plugin.has_export("main").await {
            plugin
                .call_void("main", &[])
                .await
                .map_err(|e| mofa_kernel::plugin::PluginError::ExecutionFailed(e.to_string()))?;
        } else {
            return Err(mofa_kernel::plugin::PluginError::ExecutionFailed(
                "WASM sandbox module must export '_start' or 'main'".to_string(),
            ));
        }

        #[cfg(unix)]
        use std::os::unix::process::ExitStatusExt;
        #[cfg(windows)]
        use std::os::windows::process::ExitStatusExt;

        #[cfg(unix)]
        let status = std::process::ExitStatus::from_raw(0);
        #[cfg(windows)]
        let status = std::process::ExitStatus::from_raw(0);

        Ok(std::process::Output {
            status,
            stdout: format!("Executed WASM module: {}", path.display()).into_bytes(),
            stderr: Vec::new(),
        })
    }
}

#[async_trait::async_trait]
impl ToolExecutor for ShellCommandTool {
    fn definition(&self) -> &ToolDefinition {
        &self.definition
    }

    async fn execute(&self, arguments: serde_json::Value) -> PluginResult<serde_json::Value> {
        let command = arguments["command"].as_str().ok_or_else(|| {
            mofa_kernel::plugin::PluginError::ExecutionFailed("Command is required".to_string())
        })?;

        if !self.is_command_allowed(command) {
            return Err(mofa_kernel::plugin::PluginError::ExecutionFailed(format!(
                "Command '{}' is not in the allowed commands list. Allowed: {:?}",
                command, self.allowed_commands
            )));
        }

        let args: Vec<String> = arguments
            .get("args")
            .and_then(|a| a.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(|s| s.to_string()))
                    .collect()
            })
            .unwrap_or_default();

        let working_dir = self.resolve_working_dir(&arguments)?;
        let output = if self.sandbox_enabled_for_call(&arguments) {
            self.execute_sandboxed(command, &args, &working_dir).await?
        } else {
            self.execute_host(command, &args, &working_dir).await?
        };

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();

        Ok(json!({
            "success": output.status.success(),
            "exit_code": output.status.code(),
            "stdout": Self::truncate_utf8(stdout, self.sandbox.max_output_chars),
            "stderr": Self::truncate_utf8(stderr, self.sandbox.max_output_chars),
            "sandbox": self.sandbox_enabled_for_call(&arguments)
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shell_config_enables_sandbox() {
        let tool = ShellCommandTool::new_with_defaults_from_config(&json!({
            "sandbox": true,
            "timeout_ms": 1500,
            "memory_limit_mb": 64,
            "cpu_time_limit_secs": 2,
            "max_output_chars": 100,
            "allowed_working_dirs": ["."]
        }));

        assert_eq!(tool.sandbox.mode, ShellExecutionMode::Sandbox);
        assert_eq!(tool.sandbox.timeout_ms, 1500);
        assert_eq!(tool.sandbox.memory_limit_mb, Some(64));
        assert_eq!(tool.sandbox.cpu_time_limit_secs, Some(2));
        assert_eq!(tool.sandbox.max_output_chars, 100);
        assert_eq!(tool.sandbox.allowed_working_dirs.len(), 1);
    }

    #[test]
    fn working_dir_restriction_blocks_outside_paths() {
        let mut sandbox = ShellSandboxConfig {
            mode: ShellExecutionMode::Sandbox,
            ..Default::default()
        };
        sandbox.allowed_working_dirs = vec![PathBuf::from("/tmp")];

        let tool = ShellCommandTool::new_with_config(vec!["ls".to_string()], sandbox);
        let result = tool.ensure_working_dir_allowed(Path::new("/etc"));

        assert!(result.is_err());
    }

    #[test]
    fn per_call_cannot_disable_sandbox_when_globally_enabled() {
        let tool = ShellCommandTool::new_with_config(
            vec!["echo".to_string()],
            ShellSandboxConfig {
                mode: ShellExecutionMode::Sandbox,
                ..Default::default()
            },
        );

        assert!(tool.sandbox_enabled_for_call(&json!({"sandbox": false})));
    }
}
