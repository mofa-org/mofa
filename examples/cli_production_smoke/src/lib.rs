#![allow(missing_docs)]

use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus};
use tempfile::TempDir;

pub type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

macro_rules! bail {
    ($($arg:tt)*) => {
        return Err(format!($($arg)*).into())
    };
}

pub trait ContextExt<T, E> {
    fn with_context<C, F>(self, f: F) -> Result<T>
    where
        C: std::fmt::Display + Send + Sync + 'static,
        F: FnOnce() -> C;
}

impl<T, E: std::error::Error + 'static> ContextExt<T, E> for std::result::Result<T, E> {
    fn with_context<C, F>(self, f: F) -> Result<T>
    where
        C: std::fmt::Display + Send + Sync + 'static,
        F: FnOnce() -> C,
    {
        self.map_err(|e| -> Box<dyn std::error::Error> { format!("{}: {}", f(), e).into() })
    }
}

pub const DEFAULT_AGENT_ID: &str = "smoke-agent-1";
pub const DEFAULT_SESSION_ID: &str = "smoke-session-1";

#[derive(Debug)]
pub struct CommandOutput {
    pub args: Vec<String>,
    pub status: ExitStatus,
    pub stdout: String,
    pub stderr: String,
}

impl CommandOutput {
    pub fn success(&self) -> bool {
        self.status.success()
    }

    pub fn command_line(&self) -> String {
        self.args.join(" ")
    }
}

#[derive(Debug)]
pub struct SmokeEnvironment {
    _temp_dir: TempDir,
    pub mofa_bin: PathBuf,
    pub xdg_config_home: PathBuf,
    pub xdg_data_home: PathBuf,
    pub xdg_cache_home: PathBuf,
}

impl SmokeEnvironment {
    pub fn new() -> Result<Self> {
        let mofa_bin = resolve_mofa_bin()?;
        Self::new_with_bin(mofa_bin)
    }

    pub fn new_with_bin(mofa_bin: PathBuf) -> Result<Self> {
        validate_mofa_bin(&mofa_bin)?;

        let temp_dir = TempDir::new().map_err(|e| -> Box<dyn std::error::Error> {
            format!("Failed to create temp directory for smoke run: {}", e).into()
        })?;
        let xdg_config_home = temp_dir.path().join("xdg-config");
        let xdg_data_home = temp_dir.path().join("xdg-data");
        let xdg_cache_home = temp_dir.path().join("xdg-cache");

        fs::create_dir_all(&xdg_config_home)
            .with_context(|| format!("Failed to create {}", xdg_config_home.display()))?;
        fs::create_dir_all(&xdg_data_home)
            .with_context(|| format!("Failed to create {}", xdg_data_home.display()))?;
        fs::create_dir_all(&xdg_cache_home)
            .with_context(|| format!("Failed to create {}", xdg_cache_home.display()))?;

        Ok(Self {
            _temp_dir: temp_dir,
            mofa_bin,
            xdg_config_home,
            xdg_data_home,
            xdg_cache_home,
        })
    }

    pub fn temp_root(&self) -> &Path {
        self._temp_dir.path()
    }

    pub fn mofa_data_dir(&self) -> PathBuf {
        self.xdg_data_home.join("mofa")
    }

    pub fn run(&self, args: &[&str]) -> Result<CommandOutput> {
        let output = Command::new(&self.mofa_bin)
            .args(args)
            .current_dir(workspace_root())
            .env("XDG_CONFIG_HOME", &self.xdg_config_home)
            .env("XDG_DATA_HOME", &self.xdg_data_home)
            .env("XDG_CACHE_HOME", &self.xdg_cache_home)
            .env("APPDATA", &self.xdg_config_home)
            .env("LOCALAPPDATA", &self.xdg_data_home)
            .env("MOFA_CONFIG_DIR", self.xdg_config_home.join("mofa"))
            .env("MOFA_DATA_DIR", self.xdg_data_home.join("mofa"))
            .env("MOFA_CACHE_DIR", self.xdg_cache_home.join("mofa"))
            .env("NO_COLOR", "1")
            .env("CLICOLOR", "0")
            .output()
            .with_context(|| format!("Failed to run '{}'", self.mofa_bin.display()))?;

        Ok(CommandOutput {
            args: args.iter().map(|s| s.to_string()).collect(),
            status: output.status,
            stdout: String::from_utf8_lossy(&output.stdout).to_string(),
            stderr: String::from_utf8_lossy(&output.stderr).to_string(),
        })
    }
}

#[derive(Debug, Default)]
pub struct SmokeReport {
    pub steps: Vec<StepResult>,
}

impl SmokeReport {
    pub fn passed_count(&self) -> usize {
        self.steps.iter().filter(|s| s.passed).count()
    }

    pub fn failed_count(&self) -> usize {
        self.steps.len().saturating_sub(self.passed_count())
    }

    pub fn all_passed(&self) -> bool {
        self.failed_count() == 0
    }

    fn record_step(&mut self, name: &'static str, result: Result<()>) {
        match result {
            Ok(()) => self.steps.push(StepResult {
                name,
                passed: true,
                error: None,
            }),
            Err(err) => self.steps.push(StepResult {
                name,
                passed: false,
                error: Some(format!("{err:#}")),
            }),
        }
    }
}

#[derive(Debug)]
pub struct StepResult {
    pub name: &'static str,
    pub passed: bool,
    pub error: Option<String>,
}

pub fn run_full_smoke() -> Result<SmokeReport> {
    let env = SmokeEnvironment::new()?;
    Ok(run_full_smoke_with_env(&env))
}

pub fn run_full_smoke_with_env(env: &SmokeEnvironment) -> SmokeReport {
    let mut report = SmokeReport::default();

    report.record_step("Top-level CLI commands", smoke_top_level_commands(env));
    report.record_step("Tool commands", smoke_tool_commands(env));
    report.record_step("Plugin lifecycle", smoke_plugin_lifecycle(env));
    report.record_step(
        "Agent lifecycle",
        smoke_agent_lifecycle(env, DEFAULT_AGENT_ID),
    );
    report.record_step(
        "Session lifecycle",
        smoke_session_lifecycle(env, DEFAULT_SESSION_ID),
    );
    report.record_step(
        "Negative checks",
        smoke_deleted_session_show_fails(env, DEFAULT_SESSION_ID),
    );

    report
}

pub fn smoke_top_level_commands(env: &SmokeEnvironment) -> Result<()> {
    let info = env.run(&["info"])?;
    expect_ok(&info, "mofa info")?;
    expect_stdout_contains(&info, "MoFA", "mofa info")?;

    let config_path = env.run(&["config", "path"])?;
    expect_ok(&config_path, "mofa config path")?;
    expect_stdout_contains(&config_path, "mofa", "mofa config path")?;

    let config_list = env.run(&["config", "list"])?;
    expect_ok(&config_list, "mofa config list")?;
    expect_stdout_contains(&config_list, "Global configuration", "mofa config list")?;

    Ok(())
}

pub fn smoke_tool_commands(env: &SmokeEnvironment) -> Result<()> {
    let list = env.run(&["tool", "list"])?;
    expect_ok(&list, "mofa tool list")?;
    expect_stdout_contains(&list, "echo", "mofa tool list")?;

    let info = env.run(&["tool", "info", "echo"])?;
    expect_ok(&info, "mofa tool info echo")?;
    expect_stdout_contains(&info, "Tool information", "mofa tool info echo")?;
    expect_stdout_contains(&info, "echo", "mofa tool info echo")?;

    Ok(())
}

pub fn smoke_plugin_lifecycle(env: &SmokeEnvironment) -> Result<()> {
    let initial_list = env.run(&["plugin", "list"])?;
    expect_ok(&initial_list, "mofa plugin list")?;
    expect_stdout_contains(&initial_list, "http-plugin", "mofa plugin list")?;

    let info = env.run(&["plugin", "info", "http-plugin"])?;
    expect_ok(&info, "mofa plugin info http-plugin")?;
    expect_stdout_contains(&info, "Plugin information", "mofa plugin info http-plugin")?;

    let uninstall = env.run(&["plugin", "uninstall", "http-plugin", "--force"])?;
    expect_ok(&uninstall, "mofa plugin uninstall http-plugin --force")?;

    let after_uninstall = env.run(&["plugin", "list"])?;
    expect_ok(&after_uninstall, "mofa plugin list (after uninstall)")?;
    expect_stdout_not_contains(
        &after_uninstall,
        "http-plugin",
        "mofa plugin list (after uninstall)",
    )?;

    // Re-install the plugin and verify it's back
    let install = env.run(&["plugin", "install", "http-plugin"])?;
    expect_ok(&install, "mofa plugin install http-plugin")?;
    expect_stdout_contains(
        &install,
        "installed successfully",
        "mofa plugin install http-plugin",
    )?;

    let after_install = env.run(&["plugin", "list"])?;
    expect_ok(&after_install, "mofa plugin list (after reinstall)")?;
    expect_stdout_contains(
        &after_install,
        "http-plugin",
        "mofa plugin list (after reinstall)",
    )?;

    // Installing a duplicate should fail
    let dup_install = env.run(&["plugin", "install", "http-plugin"])?;
    expect_fail(&dup_install, "mofa plugin install http-plugin (duplicate)")?;

    Ok(())
}

pub fn smoke_agent_lifecycle(env: &SmokeEnvironment, agent_id: &str) -> Result<()> {
    let start = env.run(&["agent", "start", agent_id, "--type", "cli-base"])?;
    expect_ok(&start, "mofa agent start")?;
    expect_output_contains(&start, "started", "mofa agent start")?;

    let status = env.run(&["agent", "status", agent_id])?;
    expect_ok(&status, "mofa agent status")?;
    expect_output_contains(&status, "State:", "mofa agent status")?;
    expect_output_not_contains(&status, "not found", "mofa agent status")?;

    let list = env.run(&["agent", "list"])?;
    expect_ok(&list, "mofa agent list")?;
    expect_stdout_contains(&list, agent_id, "mofa agent list")?;

    let restart = env.run(&["agent", "restart", agent_id])?;
    expect_ok(&restart, "mofa agent restart")?;
    expect_output_contains(&restart, "restarted", "mofa agent restart")?;

    let stop = env.run(&["agent", "stop", agent_id, "--force-persisted-stop"])?;
    expect_ok(&stop, "mofa agent stop --force-persisted-stop")?;
    if !(stop.stdout.contains("stopped and unregistered")
        || stop.stdout.contains("updated persisted state to Stopped"))
    {
        return Err(format!(
            "mofa agent stop output did not confirm stopped transition.\nstdout:\n{}\nstderr:\n{}",
            stop.stdout, stop.stderr
        )
        .into());
    }

    let running_only = env.run(&["agent", "list", "--running"])?;
    expect_ok(&running_only, "mofa agent list --running")?;
    expect_stdout_not_contains(&running_only, agent_id, "mofa agent list --running")?;

    Ok(())
}

pub fn smoke_session_lifecycle(env: &SmokeEnvironment, session_id: &str) -> Result<()> {
    write_session_fixture(env, session_id)?;

    let list = env.run(&["session", "list"])?;
    expect_ok(&list, "mofa session list")?;
    expect_stdout_contains(&list, session_id, "mofa session list")?;

    let show = env.run(&["session", "show", session_id, "--format", "json"])?;
    expect_ok(&show, "mofa session show --format json")?;
    let shown_json = extract_json_payload(&show).map_err(|e| -> Box<dyn std::error::Error> {
        format!("Failed to parse session show JSON output: {}", e).into()
    })?;
    let shown_id = shown_json
        .get("session_id")
        .and_then(Value::as_str)
        .unwrap_or_default();
    if shown_id != session_id {
        return Err(format!(
            "mofa session show returned unexpected session_id: expected '{session_id}', got '{shown_id}'"
        ).into());
    }

    let export_file = env.temp_root().join("session-export.json");
    let export_file_arg = export_file.to_string_lossy().to_string();
    let export = env.run(&[
        "session",
        "export",
        session_id,
        "--output",
        &export_file_arg,
        "--format",
        "json",
    ])?;
    expect_ok(&export, "mofa session export")?;

    let exported_raw = fs::read_to_string(&export_file)
        .with_context(|| format!("Failed to read {}", export_file.display()))?;
    let exported_json: Value = serde_json::from_str(&exported_raw)
        .with_context(|| format!("Failed to parse {} as JSON", export_file.display()))?;
    let exported_id = exported_json
        .get("session_id")
        .and_then(Value::as_str)
        .unwrap_or_default();
    if exported_id != session_id {
        return Err(format!(
            "mofa session export returned unexpected session_id: expected '{session_id}', got '{exported_id}'"
        ).into());
    }

    let delete = env.run(&["session", "delete", session_id, "--force"])?;
    expect_ok(&delete, "mofa session delete --force")?;

    Ok(())
}

pub fn smoke_deleted_session_show_fails(env: &SmokeEnvironment, session_id: &str) -> Result<()> {
    let deleted_show = env.run(&["session", "show", session_id])?;
    expect_fail(&deleted_show, "mofa session show (after delete)")?;
    expect_output_contains(
        &deleted_show,
        "not found",
        "mofa session show (after delete)",
    )?;
    Ok(())
}

pub fn write_session_fixture(env: &SmokeEnvironment, session_id: &str) -> Result<PathBuf> {
    let sessions_dir = env.mofa_data_dir().join("sessions");
    fs::create_dir_all(&sessions_dir)
        .with_context(|| format!("Failed to create {}", sessions_dir.display()))?;

    let fixture_path = sessions_dir.join(format!("{session_id}.jsonl"));
    let fixture = format!(
        concat!(
            "{{\"key\":\"{session_id}\",\"created_at\":\"2026-01-01T00:00:00Z\",\"updated_at\":\"2026-01-01T00:00:01Z\",\"metadata\":{{}}}}\n",
            "{{\"role\":\"user\",\"content\":\"hello from smoke test\",\"timestamp\":\"2026-01-01T00:00:01Z\"}}\n",
            "{{\"role\":\"assistant\",\"content\":\"hello back\",\"timestamp\":\"2026-01-01T00:00:02Z\"}}\n"
        ),
        session_id = session_id
    );

    fs::write(&fixture_path, fixture)
        .with_context(|| format!("Failed to write {}", fixture_path.display()))?;

    Ok(fixture_path)
}

pub fn expect_ok(output: &CommandOutput, label: &str) -> Result<()> {
    if output.success() {
        return Ok(());
    }

    return Err(format!(
        "{label} failed unexpectedly (cmd: {}).\nstdout:\n{}\nstderr:\n{}",
        output.command_line(),
        output.stdout,
        output.stderr
    )
    .into());
}

pub fn expect_fail(output: &CommandOutput, label: &str) -> Result<()> {
    if !output.success() {
        return Ok(());
    }

    return Err(format!(
        "{label} succeeded unexpectedly (cmd: {}).\nstdout:\n{}\nstderr:\n{}",
        output.command_line(),
        output.stdout,
        output.stderr
    )
    .into());
}

pub fn expect_stdout_contains(output: &CommandOutput, needle: &str, label: &str) -> Result<()> {
    if output.stdout.contains(needle) {
        return Ok(());
    }

    return Err(format!(
        "{label} output did not contain '{needle}'.\nstdout:\n{}\nstderr:\n{}",
        output.stdout, output.stderr
    )
    .into());
}

pub fn expect_stdout_not_contains(output: &CommandOutput, needle: &str, label: &str) -> Result<()> {
    if !output.stdout.contains(needle) {
        return Ok(());
    }

    return Err(format!(
        "{label} output unexpectedly contained '{needle}'.\nstdout:\n{}\nstderr:\n{}",
        output.stdout, output.stderr
    )
    .into());
}

pub fn expect_output_contains(output: &CommandOutput, needle: &str, label: &str) -> Result<()> {
    if output.stdout.contains(needle) || output.stderr.contains(needle) {
        return Ok(());
    }

    return Err(format!(
        "{label} output did not contain '{needle}' in stdout/stderr.\nstdout:\n{}\nstderr:\n{}",
        output.stdout, output.stderr
    )
    .into());
}

pub fn expect_output_not_contains(output: &CommandOutput, needle: &str, label: &str) -> Result<()> {
    if !output.stdout.contains(needle) && !output.stderr.contains(needle) {
        return Ok(());
    }

    return Err(format!(
        "{label} output unexpectedly contained '{needle}' in stdout/stderr.\nstdout:\n{}\nstderr:\n{}",
        output.stdout,
        output.stderr
    ).into());
}

pub fn extract_json_payload(output: &CommandOutput) -> Result<Value> {
    let start = output
        .stdout
        .find('{')
        .ok_or_else(|| -> Box<dyn std::error::Error> {
            format!("Could not find JSON object in output").into()
        })?;
    let payload = &output.stdout[start..];
    // Use a streaming deserializer so any trailing text after the JSON object is ignored.
    let mut iter = serde_json::Deserializer::from_str(payload).into_iter::<Value>();
    iter.next()
        .ok_or_else(|| -> Box<dyn std::error::Error> {
            format!("Empty JSON stream in output").into()
        })?
        .map_err(|e| -> Box<dyn std::error::Error> {
            format!("Invalid JSON payload: {}", e).into()
        })
}

pub fn resolve_mofa_bin() -> Result<PathBuf> {
    if let Ok(raw) = std::env::var("MOFA_BIN") {
        let raw = raw.trim();
        if !raw.is_empty() {
            return validate_mofa_bin(Path::new(raw));
        }
    }

    let fallback = workspace_root()
        .join("target")
        .join("debug")
        .join(binary_name("mofa"));

    if fallback.exists() {
        return Ok(fallback);
    }

    return Err(format!(
        "Could not find mofa binary. Build it with `cargo build -p mofa-cli` from repo root, \
         or set MOFA_BIN to the binary path. Expected fallback: {}",
        fallback.display()
    )
    .into());
}

fn validate_mofa_bin(path: impl AsRef<Path>) -> Result<PathBuf> {
    let path = path.as_ref();
    if path.is_file() {
        return Ok(path.to_path_buf());
    }

    return Err(format!(
        "Could not find mofa binary at '{}'. Build with `cargo build -p mofa-cli` or set MOFA_BIN to a valid binary path.",
        path.display()
    ).into());
}

fn binary_name(base: &str) -> String {
    if cfg!(windows) {
        format!("{base}.exe")
    } else {
        base.to_owned()
    }
}

pub fn workspace_root() -> PathBuf {
    // Allow explicit override (e.g. from tests or CI pipelines).
    if let Ok(root) = std::env::var("MOFA_WORKSPACE_ROOT") {
        return PathBuf::from(root);
    }

    // Walk upward from CARGO_MANIFEST_DIR and keep the outermost ancestor whose Cargo.toml
    // declares `[workspace]`.  This is more robust than a hardcoded ancestor depth: the crate
    // can be moved inside the repo without breaking the search.
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let mut outermost_workspace: Option<PathBuf> = None;
    for ancestor in manifest_dir.ancestors() {
        let cargo_toml = ancestor.join("Cargo.toml");
        if cargo_toml.is_file() {
            if let Ok(text) = std::fs::read_to_string(&cargo_toml) {
                // Match only a bare `[workspace]` section header (not inside a comment or
                // string literal), to avoid false positives from `workspace = true` lines.
                if text.lines().any(|line| line.trim() == "[workspace]") {
                    outermost_workspace = Some(ancestor.to_path_buf());
                }
            }
        }
    }

    // Fall back to the original hardcoded depth if the search finds nothing.
    outermost_workspace.unwrap_or_else(|| {
        manifest_dir
            .ancestors()
            .nth(2)
            .expect("workspace root should exist")
            .to_path_buf()
    })
}
