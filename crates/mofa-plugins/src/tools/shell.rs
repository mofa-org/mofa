use super::*;
use serde_json::json;
use tokio::process::Command;

/// Shell 命令工具 - 执行系统命令（受限）
/// Shell command tool - Execute system commands (restricted)
pub struct ShellCommandTool {
    definition: ToolDefinition,
    allowed_commands: Vec<String>,
}

impl ShellCommandTool {
    pub fn new(allowed_commands: Vec<String>) -> Self {
        Self {
            definition: ToolDefinition {
                name: "shell".to_string(),
                description:
                    "Execute shell commands. Only whitelisted commands are allowed for security."
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
                        }
                    },
                    "required": ["command"]
                }),
                requires_confirmation: true,
            },
            allowed_commands,
        }
    }

    /// Create with default allowed commands.
    ///
    /// SECURITY: `find`, `xargs`, `awk`, `perl`, `python*`, `ruby`, `env`, and
    /// `bash`/`sh`/`zsh` are intentionally excluded because they can spawn
    /// arbitrary sub-processes (e.g., `find -exec`, `xargs sh`, `awk system()`).
    pub fn new_with_defaults() -> Self {
        Self::new(vec![
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
        ])
    }

    fn is_command_allowed(&self, command: &str) -> bool {
        if self.allowed_commands.is_empty() {
            return false; // Default deny if no whitelist
        }
        // SECURITY: Only exact-match the base command name. The previous
        // `starts_with("{cmd} ")` check allowed embedding extra commands
        // after a space; we now require the command field to be a bare binary name.
        self.allowed_commands.iter().any(|allowed| command == allowed)
    }

    /// SECURITY: Reject arguments that contain shell meta-characters or
    /// sub-process invocation flags. This is a defence-in-depth measure:
    /// even if the base command is whitelisted, a crafted argument list
    /// must not be able to escape into a shell.
    fn sanitize_args(args: &[String]) -> Result<(), String> {
        // Characters that can trigger shell expansion or command chaining
        // when a parent shell is involved, or that have special semantics
        // in commands like `find -exec`.
        const DANGEROUS_CHARS: &[char] = &[
            '|', ';', '&', '$', '`', '>', '<', '(', ')', '{', '}', '\n',
        ];

        // Flags that allow sub-process execution in common Unix utilities.
        const DANGEROUS_FLAGS: &[&str] = &[
            "-exec", "-execdir", "-ok", "-okdir",  // find
            "--exec",                                 // various
        ];

        for (i, arg) in args.iter().enumerate() {
            // Reject any argument containing dangerous shell characters.
            if let Some(ch) = arg.chars().find(|c| DANGEROUS_CHARS.contains(c)) {
                return Err(format!(
                    "Security policy violation: argument [{}] ('{}') contains \
                     forbidden character '{}'.",
                    i, arg, ch
                ));
            }

            // Reject known sub-process execution flags (case-insensitive).
            let lower = arg.to_ascii_lowercase();
            if DANGEROUS_FLAGS.iter().any(|f| lower == *f) {
                return Err(format!(
                    "Security policy violation: argument [{}] ('{}') is a \
                     forbidden execution flag.",
                    i, arg
                ));
            }
        }

        Ok(())
    }
}

#[async_trait::async_trait]
impl ToolExecutor for ShellCommandTool {
    fn definition(&self) -> &ToolDefinition {
        &self.definition
    }

    async fn execute(&self, arguments: serde_json::Value) -> PluginResult<serde_json::Value> {
        let command = arguments["command"]
            .as_str()
            .ok_or_else(|| mofa_kernel::plugin::PluginError::ExecutionFailed("Command is required".to_string()))?;

        if !self.is_command_allowed(command) {
            return Err(mofa_kernel::plugin::PluginError::ExecutionFailed(format!(
                "Command '{}' is not in the allowed commands list. Allowed: {:?}",
                command,
                self.allowed_commands
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

        // SECURITY: Sanitize every argument before spawning the process.
        Self::sanitize_args(&args).map_err(|msg| {
            mofa_kernel::plugin::PluginError::ExecutionFailed(msg)
        })?;

        let mut cmd = Command::new(command);
        cmd.args(&args);

        if let Some(dir) = arguments.get("working_dir").and_then(|d| d.as_str()) {
            cmd.current_dir(dir);
        }

        let output = cmd.output().await?;

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();

        Ok(json!({
            "success": output.status.success(),
            "exit_code": output.status.code(),
            "stdout": if stdout.len() > 5000 { format!("{}...[truncated]", &stdout[..5000]) } else { stdout },
            "stderr": if stderr.len() > 5000 { format!("{}...[truncated]", &stderr[..5000]) } else { stderr }
        }))
    }
}
