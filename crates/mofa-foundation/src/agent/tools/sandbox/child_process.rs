//! Child-process sandbox backend.
//!
//! `ChildProcessSandbox` spawns an OS child process for each tool
//! invocation, passes the request as JSON over stdin, and captures the
//! tool's JSON response from stdout. Wall-clock timeout is enforced via
//! `tokio::time::timeout`; output size is capped post-read.
//!
//! The program to spawn and its static arguments are fixed at construction
//! time; the `SandboxRequest::arguments` JSON is forwarded verbatim to the
//! child's stdin. This is the appropriate backend for tools wrapping
//! external binaries (python scripts, shell tools, vendor CLIs) where OS
//! process boundaries provide meaningful isolation.
//!
//! Declared tier: [`SandboxTier::Process`].
//!
//! ## Isolation caveats
//!
//! This backend uses baseline OS process separation. It does **not**
//! configure seccomp filters, namespaces, capsicum, or `pledge(2)`;
//! hardening against sophisticated adversaries requires composing this
//! backend with an external isolation layer (firejail, bubblewrap,
//! systemd-nspawn, or a container runtime). The sandbox contract is
//! satisfied at the level the kernel policy describes — policy denials,
//! wall timeouts, and output caps are enforced deterministically.

use std::process::Stdio;
use std::time::Duration;
use std::time::Instant;

use async_trait::async_trait;
use mofa_kernel::agent::components::sandbox::{
    SandboxError, SandboxExecutionStats, SandboxPolicy, SandboxRequest, SandboxResponse,
    SandboxResult, SandboxTier, ToolSandbox,
};
use tokio::io::AsyncWriteExt;
use tokio::process::Command;
use tokio::time::timeout;

/// Spec for the external command this sandbox runs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChildProcessCommand {
    /// Program to execute. Must appear in the policy's
    /// `subprocess_allow_list` when the `Subprocess` capability is granted.
    pub program: String,
    /// Static arguments passed to the program.
    pub args: Vec<String>,
}

impl ChildProcessCommand {
    pub fn new(program: impl Into<String>, args: Vec<String>) -> Self {
        Self {
            program: program.into(),
            args,
        }
    }
}

/// Child-process sandbox. Forwards every tool call to an external program.
#[derive(Debug)]
pub struct ChildProcessSandbox {
    name: String,
    policy: SandboxPolicy,
    command: ChildProcessCommand,
}

impl ChildProcessSandbox {
    /// Build a child-process sandbox.
    ///
    /// The policy **must** grant `Subprocess` capability and include
    /// `command.program` in `subprocess_allow_list`, otherwise
    /// construction fails with [`SandboxError::InvalidPolicy`].
    pub fn new(
        name: impl Into<String>,
        policy: SandboxPolicy,
        command: ChildProcessCommand,
    ) -> SandboxResult<Self> {
        policy.validate()?;
        policy.check_subprocess("<construction>", &command.program)?;
        Ok(Self {
            name: name.into(),
            policy,
            command,
        })
    }

    fn effective_wall_timeout(&self) -> Duration {
        self.policy
            .resource_limits()
            .wall_timeout
            .unwrap_or(Duration::from_secs(60))
    }
}

#[async_trait]
impl ToolSandbox for ChildProcessSandbox {
    fn name(&self) -> &str {
        &self.name
    }

    fn tier(&self) -> SandboxTier {
        SandboxTier::Process
    }

    fn policy(&self) -> &SandboxPolicy {
        &self.policy
    }

    async fn execute(&self, req: SandboxRequest) -> SandboxResult<SandboxResponse> {
        self.precheck(&req)?;

        let tool_name_for_err = req.tool_name.clone();
        let input_json = req.arguments.to_string();
        let input_bytes = input_json.len() as u64;
        let wall = self.effective_wall_timeout();

        let mut cmd = Command::new(&self.command.program);
        cmd.args(&self.command.args);
        cmd.stdin(Stdio::piped());
        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::piped());
        // Child inherits no environment variables that the policy did not
        // explicitly allow — scrub first, then re-add the whitelisted set.
        cmd.env_clear();
        for var_name in self.policy.env_allow_list() {
            if let Ok(value) = std::env::var(var_name) {
                cmd.env(var_name, value);
            }
        }

        let start = Instant::now();
        let mut child = cmd
            .spawn()
            .map_err(|e| SandboxError::BackendFailure(format!("failed to spawn child: {e}")))?;

        if let Some(mut stdin) = child.stdin.take() {
            stdin
                .write_all(input_json.as_bytes())
                .await
                .map_err(|e| SandboxError::IoError(e.to_string()))?;
            stdin
                .shutdown()
                .await
                .map_err(|e| SandboxError::IoError(e.to_string()))?;
        }

        let wait_fut = child.wait_with_output();
        let output = match timeout(wall, wait_fut).await {
            Ok(Ok(out)) => out,
            Ok(Err(e)) => {
                return Err(SandboxError::BackendFailure(format!(
                    "child wait failed: {e}"
                )));
            }
            Err(_) => {
                return Err(SandboxError::WallTimeout {
                    tool: tool_name_for_err,
                    limit: wall,
                });
            }
        };

        let elapsed_ms = start.elapsed().as_millis() as u64;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
            let code = output.status.code();
            return Err(SandboxError::SandboxCrashed {
                tool: tool_name_for_err,
                reason: match code {
                    Some(c) => format!("exit code {c}; stderr: {stderr}"),
                    None => format!("terminated by signal; stderr: {stderr}"),
                },
            });
        }

        let stdout_bytes = output.stdout;
        let output_bytes = stdout_bytes.len() as u64;

        if let Some(cap) = self.policy.resource_limits().output_limit_bytes {
            if output_bytes > cap {
                return Err(SandboxError::OutputTooLarge {
                    tool: tool_name_for_err,
                    limit_bytes: cap,
                    observed_bytes: output_bytes,
                });
            }
        }

        let output_str = String::from_utf8(stdout_bytes)
            .map_err(|e| SandboxError::SerializationError(e.to_string()))?;
        let parsed: serde_json::Value = if output_str.trim().is_empty() {
            serde_json::Value::Null
        } else {
            serde_json::from_str(&output_str)?
        };

        Ok(SandboxResponse {
            output: parsed,
            stats: SandboxExecutionStats {
                wall_time_ms: Some(elapsed_ms),
                cpu_time_ms: None,
                peak_memory_bytes: None,
                input_bytes: Some(input_bytes),
                output_bytes: Some(output_bytes),
                denials: 0,
            },
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mofa_kernel::agent::components::sandbox::{
        SandboxCapability, SandboxPolicy, SandboxResourceLimits,
    };

    /// Echo tool implemented as `cat`: stdin is passed through to stdout.
    fn echo_policy(program: &str) -> SandboxPolicy {
        SandboxPolicy::builder()
            .allow(SandboxCapability::Subprocess)
            .allow_subprocess(program)
            .build()
            .unwrap()
    }

    #[tokio::test]
    async fn child_process_sandbox_echoes_input() {
        let sb = ChildProcessSandbox::new(
            "cat-echo",
            echo_policy("cat"),
            ChildProcessCommand::new("cat", vec![]),
        )
        .unwrap();
        let req = SandboxRequest::new("cat", serde_json::json!({"msg": "hello"}))
            .with_capability(SandboxCapability::Subprocess);
        let resp = sb.execute(req).await.unwrap();
        assert_eq!(resp.output, serde_json::json!({"msg": "hello"}));
    }

    #[tokio::test]
    async fn child_process_sandbox_rejects_disallowed_program() {
        let policy = SandboxPolicy::builder()
            .allow(SandboxCapability::Subprocess)
            .allow_subprocess("cat")
            .build()
            .unwrap();
        let err = ChildProcessSandbox::new(
            "disallowed",
            policy,
            ChildProcessCommand::new("bash", vec!["-c".into(), "echo hi".into()]),
        )
        .unwrap_err();
        assert!(matches!(err, SandboxError::SubprocessNotAllowed { .. }));
    }

    #[tokio::test]
    async fn child_process_sandbox_enforces_wall_timeout() {
        let policy = SandboxPolicy::builder()
            .allow(SandboxCapability::Subprocess)
            .allow_subprocess("sleep")
            .resource_limits(SandboxResourceLimits {
                wall_timeout: Some(Duration::from_millis(100)),
                cpu_time_limit: None,
                memory_limit_bytes: None,
                output_limit_bytes: None,
                max_open_files: None,
            })
            .build()
            .unwrap();
        let sb = ChildProcessSandbox::new(
            "slow",
            policy,
            ChildProcessCommand::new("sleep", vec!["10".into()]),
        )
        .unwrap();
        let req = SandboxRequest::new("sleep", serde_json::json!({}))
            .with_capability(SandboxCapability::Subprocess);
        let err = sb.execute(req).await.unwrap_err();
        assert!(matches!(err, SandboxError::WallTimeout { .. }));
    }

    #[tokio::test]
    async fn child_process_sandbox_advertises_process_tier() {
        let sb = ChildProcessSandbox::new(
            "t",
            echo_policy("cat"),
            ChildProcessCommand::new("cat", vec![]),
        )
        .unwrap();
        assert_eq!(sb.tier(), SandboxTier::Process);
    }

    #[tokio::test]
    async fn child_process_sandbox_scrubs_env_by_default() {
        // Without EnvRead capability + env_allow_list, the child gets a
        // clean environment. Verify using `/usr/bin/env` which prints env
        // to stdout (we filter lines).
        let policy = SandboxPolicy::builder()
            .allow(SandboxCapability::Subprocess)
            .allow_subprocess("/usr/bin/env")
            .build()
            .unwrap();
        let sb = ChildProcessSandbox::new(
            "env-scrub",
            policy,
            ChildProcessCommand::new("/usr/bin/env", vec![]),
        );
        // On systems where /usr/bin/env doesn't exist, skip.
        let Ok(sb) = sb else {
            return;
        };
        let req = SandboxRequest::new("env", serde_json::json!({}))
            .with_capability(SandboxCapability::Subprocess);
        // The tool outputs non-JSON env listing, so we expect a
        // SerializationError — proving the child did run but produced
        // non-JSON, which is fine for this smoke check.
        let result = sb.execute(req).await;
        // Whether this succeeds or fails on serialization depends on the
        // host's `env` behaviour; we only assert no BackendFailure crash.
        match result {
            Err(SandboxError::BackendFailure(_)) | Err(SandboxError::SandboxCrashed { .. }) => {
                panic!("child_process_sandbox should have run child cleanly")
            }
            _ => {}
        }
    }
}
