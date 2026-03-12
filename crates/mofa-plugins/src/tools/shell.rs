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

    /// Create with default allowed commands
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
            "find".to_string(),
        ])
    }

    fn is_command_allowed(&self, command: &str) -> bool {
        if self.allowed_commands.is_empty() {
            return false; // Default deny if no whitelist
        }
        self.allowed_commands
            .iter()
            .any(|allowed| command == allowed || command.starts_with(&format!("{} ", allowed)))
    }

    /// Reject arguments that can escalate a whitelisted command into
    /// arbitrary code execution or destructive filesystem operations.
    ///
    /// `find` is the main concern: `-exec`, `-execdir`, `-ok`, `-okdir`
    /// hand control to an arbitrary binary, and `-delete` removes files
    /// without confirmation regardless of the `requires_confirmation` flag.
    fn reject_dangerous_args(
        command: &str,
        args: &[String],
    ) -> Result<(), mofa_kernel::plugin::PluginError> {
        const DANGEROUS_FIND_ARGS: &[&str] =
            &["-exec", "-execdir", "-ok", "-okdir", "-delete"];

        let blocklist: &[&str] = match command {
            "find" => DANGEROUS_FIND_ARGS,
            _ => return Ok(()),
        };

        for arg in args {
            if blocklist.iter().any(|blocked| arg == *blocked) {
                return Err(mofa_kernel::plugin::PluginError::ExecutionFailed(
                    format!(
                        "Argument '{}' is not allowed for '{}' — \
                         it can be used for arbitrary command execution",
                        arg, command
                    ),
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

        Self::reject_dangerous_args(command, &args)?;

        let mut cmd = Command::new(command);
        cmd.args(&args);

        if let Some(dir) = arguments.get("working_dir").and_then(|d| d.as_str()) {
            cmd.current_dir(dir);
        }

        let output = cmd.output().await?;

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();

        let truncate = |s: String, limit: usize| -> String {
            if s.len() > limit {
                // Find the last valid char boundary at or before the limit
                // to avoid panicking on multi-byte UTF-8 characters.
                let mut end = limit;
                while !s.is_char_boundary(end) {
                    end -= 1;
                }
                format!("{}...[truncated]", &s[..end])
            } else {
                s
            }
        };

        Ok(json!({
            "success": output.status.success(),
            "exit_code": output.status.code(),
            "stdout": truncate(stdout, 5000),
            "stderr": truncate(stderr, 5000)
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ---- allowlist tests ----

    #[test]
    fn allowed_command_passes() {
        let tool = ShellCommandTool::new_with_defaults();
        assert!(tool.is_command_allowed("ls"));
        assert!(tool.is_command_allowed("find"));
        assert!(tool.is_command_allowed("grep"));
    }

    #[test]
    fn disallowed_command_rejected() {
        let tool = ShellCommandTool::new_with_defaults();
        assert!(!tool.is_command_allowed("rm"));
        assert!(!tool.is_command_allowed("curl"));
        assert!(!tool.is_command_allowed("bash"));
    }

    #[test]
    fn empty_allowlist_denies_everything() {
        let tool = ShellCommandTool::new(vec![]);
        assert!(!tool.is_command_allowed("ls"));
        assert!(!tool.is_command_allowed("echo"));
    }

    // ---- argument sanitization tests ----

    #[test]
    fn find_with_safe_args_allowed() {
        let args = vec![
            "/tmp".into(),
            "-name".into(),
            "*.log".into(),
            "-type".into(),
            "f".into(),
        ];
        assert!(ShellCommandTool::reject_dangerous_args("find", &args).is_ok());
    }

    #[test]
    fn find_exec_blocked() {
        let args = vec![
            ".".into(),
            "-name".into(),
            "*.key".into(),
            "-exec".into(),
            "cat".into(),
            "{}".into(),
            ";".into(),
        ];
        let err = ShellCommandTool::reject_dangerous_args("find", &args).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("-exec"), "error should mention the blocked arg");
    }

    #[test]
    fn find_execdir_blocked() {
        let args = vec![".".into(), "-execdir".into(), "rm".into(), "{}".into(), ";".into()];
        assert!(ShellCommandTool::reject_dangerous_args("find", &args).is_err());
    }

    #[test]
    fn find_ok_blocked() {
        let args = vec![".".into(), "-ok".into(), "rm".into(), "{}".into(), ";".into()];
        assert!(ShellCommandTool::reject_dangerous_args("find", &args).is_err());
    }

    #[test]
    fn find_okdir_blocked() {
        let args = vec![".".into(), "-okdir".into(), "rm".into(), "{}".into(), ";".into()];
        assert!(ShellCommandTool::reject_dangerous_args("find", &args).is_err());
    }

    #[test]
    fn find_delete_blocked() {
        let args = vec!["/tmp".into(), "-name".into(), "*.tmp".into(), "-delete".into()];
        assert!(ShellCommandTool::reject_dangerous_args("find", &args).is_err());
    }

    #[test]
    fn non_find_commands_skip_arg_checks() {
        // Even "dangerous-looking" args are fine for commands without a blocklist.
        let args = vec!["-exec".into(), "something".into()];
        assert!(ShellCommandTool::reject_dangerous_args("grep", &args).is_ok());
        assert!(ShellCommandTool::reject_dangerous_args("ls", &args).is_ok());
        assert!(ShellCommandTool::reject_dangerous_args("cat", &args).is_ok());
    }

    #[test]
    fn find_no_args_allowed() {
        let args: Vec<String> = vec![];
        assert!(ShellCommandTool::reject_dangerous_args("find", &args).is_ok());
    }

    // ---- integration: execute rejects dangerous args ----

    #[tokio::test]
    async fn execute_blocks_find_exec() {
        let tool = ShellCommandTool::new_with_defaults();
        let input = json!({
            "command": "find",
            "args": [".", "-exec", "cat", "/etc/passwd", "{}", ";"]
        });
        let result = tool.execute(input).await;
        assert!(result.is_err(), "find -exec must be rejected");
    }

    #[tokio::test]
    async fn execute_blocks_find_delete() {
        let tool = ShellCommandTool::new_with_defaults();
        let input = json!({
            "command": "find",
            "args": ["/tmp", "-name", "*.tmp", "-delete"]
        });
        let result = tool.execute(input).await;
        assert!(result.is_err(), "find -delete must be rejected");
    }

    #[tokio::test]
    async fn execute_allows_safe_find() {
        let tool = ShellCommandTool::new_with_defaults();
        let input = json!({
            "command": "find",
            "args": [".", "-maxdepth", "1", "-name", "*.rs"]
        });
        // Should succeed (or at least not fail on argument validation).
        let result = tool.execute(input).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn execute_allows_echo() {
        let tool = ShellCommandTool::new_with_defaults();
        let input = json!({
            "command": "echo",
            "args": ["hello"]
        });
        let result = tool.execute(input).await.unwrap();
        assert!(result["stdout"].as_str().unwrap().contains("hello"));
    }
}
